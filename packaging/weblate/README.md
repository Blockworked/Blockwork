# Weblate server

Blockwork's UI is translated on a self-hosted [Weblate](https://weblate.org).
Weblate edits `blockwork-qt/i18n/qml_*.ts` and opens a pull request with
the changes; the Translations workflow then compiles the `.qm` files the
build embeds.

## What you need

- A machine that stays on, with Docker and Docker Compose, at least 2 GB of
  RAM (4 GB is comfortable) and a few GB of disk.
- A domain name pointing at it, like `translate.example.com`, so
  translators can reach it and GitHub can send it webhooks.
- An SMTP account for sign-up and notification mail.
- A GitHub account for Weblate to open pull requests from. A separate bot
  account is tidiest; a classic token with the `public_repo` scope is enough.

## Setting up the server

```sh
git clone https://github.com/WeblateOrg/docker-compose.git weblate-docker
cd weblate-docker
cp /path/to/Blockwork/packaging/weblate/docker-compose.override.example.yml docker-compose.override.yml
$EDITOR docker-compose.override.yml   # fill in every CHANGE_ME
docker compose up -d
```

Weblate listens on `127.0.0.1:8080` only. Put HTTPS in front of it, for
example with [Caddy](https://caddyserver.com), which gets certificates by
itself:

```
translate.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Without a public IP, a Cloudflare Tunnel or Tailscale Funnel pointed at
`127.0.0.1:8080` does the same job.

Updating: `docker compose pull && docker compose up -d`. Back up the
`weblate-data` and `postgres-data` volumes.

## Adding Blockwork

Sign in as the admin, then:

1. **Add new translation project**: name `Blockwork`, project website
   `https://github.com/Blockworked/Blockwork`.
2. **Add new translation component** in it, from a version control system:
   - Component name: `UI`
   - Version control system: **GitHub pull request**
   - Source code repository: `https://github.com/Blockworked/Blockwork.git`
   - Repository branch: `master`
   - Push branch: `weblate`
3. On the next page:
   - File format: **Qt Linguist translation file**
   - File mask: `blockwork-qt/i18n/qml_*.ts`
   - Source language: English
   - Adding new translation: **Create new language file**
   - Language code style: **POSIX style using underscore** (`pt_BR`), which
     is what the app's loader expects.

   `qml_en.ts` only holds English plural forms and shows up as the source
   language, which translators don't edit.
4. The pirate file uses the code `pirate`, which Weblate creates as a
   language of its own. Give it a name under **Manage > Languages**.
5. In the project's **Settings > Workflow**, set the license and turn on
   **Enable hooks**.

## GitHub

- In the Blockwork repository, **Settings > Webhooks > Add webhook**:
  payload URL `https://translate.example.com/hooks/github/`, content type
  `application/json`, just the push event. Weblate then pulls new strings
  as soon as they land.
- Weblate pushes to a `weblate` branch in a fork under its GitHub account
  and opens a pull request. Merging it runs the Translations workflow,
  which commits the compiled `.qm` files.

## Updating strings

After changing UI text, run `just translations` and commit the `.ts`
files. Weblate and `lupdate` both rewrite the `.ts` files, so merge
Weblate's open pull request first (or **Repository maintenance > Commit**
and push in Weblate) to avoid conflicts.

A new language shows up in Settings by itself once its `.qm` is built in.
