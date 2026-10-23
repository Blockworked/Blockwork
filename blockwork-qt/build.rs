use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};
use qt_build_utils::{QResource, QResourceFile, QResources};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

fn main() {
    #[cfg(windows)]
    {
        // Native snap styles in src/window_frame.cpp.
        println!("cargo:rustc-link-lib=user32");
        // Explorer, taskbar grouping and the installer pick the exe's embedded
        // icon; the runtime title-bar/taskbar icon is set separately in
        // src/app_icon.cpp from the Qt resource below.
        let mut res = winres::WindowsResource::new();
        res.set_icon("../res/icons/blockwork.ico");
        res.compile().expect("failed to embed Windows icon");
    }

    // qml/module.json lists the module's files; cmake/BlockworkEditorQml.cmake
    // packages its editor half for the website from the same list.
    let qml = Manifest::read("qml/module.json");

    // Shared Lucide icons (lean set): staged under qml/Lucide/ (gitignored,
    // rewritten only when changed) and compiled into this module, so every
    // page resolves LucideIcon/Icons locally with no import.
    let (lucide_icons, lucide_item) = lucide_qt::stage_qml_here("qml/Lucide");
    assert_eq!(
        list(&qml.json["lucide"]["files"], "lucide.files"),
        [&lucide_icons, &lucide_item].map(|p| p.file_name().unwrap().to_string_lossy().into_owned()),
        "qml/module.json: `lucide.files` differs from what lucide-qt stages"
    );

    let singletons = qml.strings("singletons");
    let module = qml
        .desktop_files()
        .iter()
        .fold(
            QmlModule::new(qml.string("uri").as_str())
                .version(qml.version().0, qml.version().1)
                .qml_file(QmlFile::from(lucide_icons).singleton(true)),
            |module, name| module.qml_file(QmlFile::from(format!("qml/{name}")).singleton(singletons.contains(name))),
        )
        .qml_files([lucide_item]);

    let mut style = QResources::new();
    for (name, files) in qml.json["style_modules"].as_object().expect("`style_modules` must be an object") {
        let resource = list(files, name).iter().fold(
            QResource::new().prefix(format!("/qt/qml/{name}")),
            |resource, file| resource.file(QResourceFile::new(format!("qml/{name}/{file}")).alias(file.as_str())),
        );
        style = style.resource(resource);
    }
    // Runtime window icon (see src/app_icon.cpp, addressed as ":/icons/..."
    // in C++) beside the editor's own resources; kept under /icons so its
    // resource path stays stable.
    let mut resources = QResources::new();
    for (prefix, files) in qml.json["editor_resources"].as_object().expect("`editor_resources` must be an object") {
        let mut resource = QResource::new().prefix(prefix.as_str());
        for file in list(files, prefix) {
            let path = format!("qml/{file}");
            let alias = Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
            resource = resource.file(QResourceFile::new(path).alias(alias));
        }
        if prefix == "/icons" {
            resource = resource.file(QResourceFile::new("../res/icons/window-shadow.png").alias("window-shadow.png"));
        }
        resources = resources.resource(resource);
    }

    let builder = qml
        .strings("qt_modules")
        .iter()
        .fold(CxxQtBuilder::new_qml_module(module), |builder, qt| builder.qt_module(qt))
        .qrc_resources(style)
        .qrc_resources(resources)
    .file("src/app_bridge.rs")
    .file("src/prompt_bridge.rs")
    .file("src/translator.rs")
    .file("src/translations.rs")
    .file("src/app_icon.rs")
    .file("src/application.rs")
    .file("src/qt_diagnostics.rs")
    .file("src/window_frame.rs")
    // UI translations, built from i18n/*.ts by `just translations`.
    .qrc_resources(translation_resources())
    .qt_module("Network")
    .qt_module("Widgets");

    // QtGui private headers for the Wayland frame-margin hook (window_frame.cpp).
    let private_includes = qt_private_includes();

    unsafe {
        builder
            .cc_builder(|cc| {
                // qmlcachegen emits UTF-8 C++ string literals, including the
                // pack-review bullets. MSVC otherwise uses the system code page.
                if cc.get_compiler().is_like_msvc() {
                    cc.flag("/utf-8");
                }
                cc.include("src");
                cc.file("src/qt_diagnostics.cpp");
                cc.file("src/app_icon.cpp");
                cc.file("src/application.cpp");
                cc.file("src/window_frame.cpp");
                cc.file("src/translations.cpp");
                for dir in &private_includes {
                    cc.include(dir);
                }
            })
            .build();
    }
}

/// Every compiled translation in i18n/, served as `:/i18n/qml_<code>.qm`.
fn translation_resources() -> QResources {
    println!("cargo:rerun-if-changed=i18n");
    let mut resource = QResource::new().prefix("/i18n");
    let mut files: Vec<_> = std::fs::read_dir("i18n")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "qm"))
        .collect();
    files.sort();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        resource = resource.file(QResourceFile::new(format!("i18n/{name}")).alias(name));
    }
    QResources::new().resource(resource)
}

/// `QtCore/<ver>` and `QtGui/<ver>` private header dirs, Linux only.
fn qt_private_includes() -> Vec<std::path::PathBuf> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return Vec::new();
    }
    let Ok(qt) = qt_build_utils::QtBuild::new(vec!["Core".into(), "Gui".into()]) else {
        return Vec::new();
    };
    let version = qt.version().to_string();
    qt.include_paths()
        .into_iter()
        .filter_map(|dir| {
            let name = dir.file_name()?.to_str()?;
            (name == "QtCore" || name == "QtGui").then(|| dir.join(&version))
        })
        .filter(|dir| dir.exists())
        .collect()
}

/// `qml/module.json`: the module's files in three groups. `editor` is
/// EditorPage and everything it reaches, `host_provided` the window chrome an
/// embedding host replaces, `desktop` the rest of the app.
struct Manifest {
    json: Value,
}

fn list(value: &Value, what: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("qml/module.json: `{what}` must be a list"))
        .iter()
        .map(|v| v.as_str().unwrap_or_else(|| panic!("qml/module.json: `{what}` holds strings")).to_owned())
        .collect()
}

impl Manifest {
    fn read(path: &str) -> Self {
        println!("cargo:rerun-if-changed={path}");
        println!("cargo:rerun-if-changed=qml");
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let manifest = Self { json: serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}")) };
        manifest.check();
        manifest
    }

    fn string(&self, key: &str) -> String {
        self.json[key].as_str().unwrap_or_else(|| panic!("qml/module.json: `{key}` missing")).to_owned()
    }

    fn strings(&self, key: &str) -> Vec<String> {
        list(&self.json[key], key)
    }

    fn version(&self) -> (usize, usize) {
        self.string("version")
            .split_once('.')
            .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
            .expect("qml/module.json: `version` must be MAJOR.MINOR")
    }

    fn desktop_files(&self) -> Vec<String> {
        ["editor", "host_provided", "desktop"].iter().flat_map(|group| self.strings(group)).collect()
    }

    /// Fails the build when the manifest and qml/ disagree, so the website's
    /// CMake build can't silently miss a file the editor needs.
    fn check(&self) {
        let mut problems = Vec::new();
        let files = self.desktop_files();
        let listed: BTreeSet<&str> = files.iter().map(String::as_str).collect();
        if listed.len() != files.len() {
            problems.push("a file is listed twice".to_owned());
        }
        let on_disk = qml_names("qml");
        for name in &on_disk {
            if !listed.contains(name.as_str()) {
                problems.push(format!("{name} is in qml/ but in no group"));
            }
        }
        for name in &listed {
            if !on_disk.contains(*name) {
                problems.push(format!("{name} is listed but not in qml/"));
            }
        }
        for name in self.strings("singletons") {
            if !listed.contains(name.as_str()) {
                problems.push(format!("singleton {name} is in no group"));
            }
        }
        let styles = self.json["style_modules"].as_object().expect("`style_modules` must be an object");
        for (name, style_files) in styles {
            let mut want: Vec<String> = list(style_files, name);
            let mut have: Vec<String> = std::fs::read_dir(format!("qml/{name}"))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect();
            want.sort();
            have.sort();
            if want != have {
                problems.push(format!("style module {name} lists {want:?} but holds {have:?}"));
            }
        }

        // Every local type or module an editor file uses must be packaged with the editor.
        let editor: BTreeSet<String> = self.strings("editor").into_iter().collect();
        let reachable: BTreeSet<String> = editor.iter().chain(&self.strings("host_provided")).cloned().collect();
        for file in &editor {
            let text = std::fs::read_to_string(format!("qml/{file}")).unwrap_or_default();
            let code: String = text.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
            for name in &on_disk {
                let ty = name.trim_end_matches(".qml");
                if name != file && !reachable.contains(name) && uses_type(&code, ty) {
                    problems.push(format!("editor file {file} uses {ty}, which is not in `editor` or `host_provided`"));
                }
            }
            for line in code.lines() {
                let Some(module) = line.trim().strip_prefix("import ").and_then(|r| r.split_whitespace().next()) else {
                    continue;
                };
                if Path::new("qml").join(module).is_dir() && module != "Lucide" && !styles.contains_key(module) {
                    problems.push(format!("editor file {file} imports {module}, which is not in `style_modules`"));
                }
            }
        }
        if !problems.is_empty() {
            panic!("qml/module.json is out of step with qml/:\n  {}", problems.join("\n  "));
        }
    }
}

fn qml_names(dir: &str) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {dir}: {e}"))
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| name.ends_with(".qml"))
        .collect()
}

/// `Type {` or `Type.member` with no identifier character or dot before it.
fn uses_type(code: &str, ty: &str) -> bool {
    code.match_indices(ty).any(|(at, _)| {
        let before = code[..at].chars().next_back();
        let rest = code[at + ty.len()..].trim_start_matches([' ', '\t']);
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.')
            && (rest.starts_with('{') || rest.starts_with('.'))
    })
}
