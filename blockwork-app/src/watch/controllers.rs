//! Controller triggers consume ordered changes, including short button presses.
use super::{ReportOnce, StrandKey, Target, fires_on_change, variable_env};
use crate::{AppHandle, scheduled_run, state::SharedState};
use blockwork_core::input::{
    controllers,
    gamepad::{GamepadAxis, GamepadButton, above_threshold, deadzone},
};
use blockwork_core::macros::InstructionKind;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
#[derive(Clone, Debug)]
enum Condition {
    Button(GamepadButton, bool),
    Connected(bool),
    Axis(GamepadAxis, f64, bool),
}
struct Watched {
    target: Target,
    profile: String,
    condition: Condition,
}
fn sample(
    w: &Watched,
    devices: &[controllers::Controller],
    old: Option<bool>,
    zone: f64,
) -> Result<bool, String> {
    let device = controllers::select(devices, &w.profile)?;
    Ok(match w.condition {
        Condition::Connected(_) => device.is_some(),
        Condition::Button(button, _) => device
            .and_then(|d| d.buttons.get(&button))
            .copied()
            .unwrap_or(false),
        Condition::Axis(axis, threshold, _) => above_threshold(
            old.unwrap_or(false),
            deadzone(
                device
                    .and_then(|d| d.axes.get(&axis))
                    .copied()
                    .unwrap_or(0.0),
                zone,
            ),
            threshold,
        ),
    })
}
pub(crate) fn start(state: SharedState, app: AppHandle) {
    let _=std::thread::Builder::new().name("controller-watch".into()).spawn(move||{
        let mut subscription=None;let mut devices=vec![];let mut last:HashMap<StrandKey,bool>=HashMap::new();let mut definitions:HashMap<StrandKey,String>=HashMap::new();let mut report=ReportOnce::default();
        loop {
            std::thread::sleep(Duration::from_millis(10));
            let (watched,emulator,speed,zone)={
                let Ok(s)=state.lock() else{continue};
                if !s.controller_input_enabled || s.triggers_paused {subscription=None;last.clear();continue;}
                let Some(emulator)=s.emulator.as_ref().map(Arc::clone) else{continue};
                let mut watched=vec![];
                for mac in scheduled_run::live_macros(&s){
                    if !super::permitted(mac,"input"){continue;}
                    let env=variable_env(mac);
                    for strand in &mac.strands{
                        let Some(header)=strand.instructions.first() else{continue};
                        let (profile,condition)=match &header.kind{
                            InstructionKind::WhenGamepadButton{controller,button,pressed}=>(controller,Condition::Button(*button,*pressed)),
                            InstructionKind::WhenGamepadConnection{controller,connected}=>(controller,Condition::Connected(*connected)),
                            InstructionKind::WhenGamepadAxis{controller,axis,threshold,above}=>{
                                let Some(threshold)=super::header_value(threshold,&env).ok().and_then(|v|v.eval_number().ok()).filter(|n|n.is_finite() && (0.05..=1.0).contains(n)) else{continue};
                                (controller,Condition::Axis(*axis,threshold,*above))
                            }
                            _=>continue,
                        };
                        let Ok(profile)=super::header_value(profile,&env).and_then(|v|v.eval_text()) else{continue};
                        watched.push(Watched{target:Target::new(mac,strand),profile,condition});
                    }
                }
                (watched,emulator,s.global_speed_multiplier,s.controller_deadzone)
            };
            if watched.is_empty(){subscription=None;last.clear();continue;}
            if subscription.is_none(){
                let Some((baseline,stream))=report.check("Controller triggers",controllers::subscribe()) else{std::thread::sleep(Duration::from_millis(250));continue};
                devices=baseline; subscription=Some(stream);last.clear();
            }
            let stream=subscription.as_ref().unwrap();
            if stream.overflowed(){
                blockwork_core::run_log::error(&blockwork_core::run_log::Source::app("Controller triggers"),"Controller event queue overflowed; triggers were resynchronized without replaying stale presses");
                while stream.try_recv().is_ok(){}
                devices=controllers::list().unwrap_or_default();last.clear();
            }
            // Newly added/edited headers establish a baseline before consuming changes.
            for w in &watched {
                let signature=format!("{}:{:?}",w.profile,w.condition);
                if definitions.get(&w.target.key)!=Some(&signature) {last.remove(&w.target.key);definitions.insert(w.target.key.clone(),signature);}
                if !last.contains_key(&w.target.key) && let Ok(value)=sample(w,&devices,None,zone){last.insert(w.target.key.clone(),value);}
            }
            for _ in 0..256 {
                let Ok(event)=stream.try_recv() else{break};devices=event.controllers;
                for w in &watched {
                    let before=last.get(&w.target.key).copied();
                    let Some(now)=report.check("Controller selector",sample(w,&devices,before,zone)) else{last.remove(&w.target.key);continue};
                    let fire_on=match w.condition{Condition::Button(_,v)|Condition::Connected(v)|Condition::Axis(_,_,v)=>v};
                    if fires_on_change(before,now,fire_on){w.target.fire(&emulator,speed,&state,&app);}
                    last.insert(w.target.key.clone(),now);
                }
            }
            last.retain(|key,_|watched.iter().any(|w|&w.target.key==key));
            definitions.retain(|key,_|watched.iter().any(|w|&w.target.key==key));
        }
    });
}
