mod config;
mod config_dialog;

use aviutl2::{anyhow, tracing};
use std::collections::HashMap;

pub static EDIT_HANDLE: aviutl2::generic::GlobalEditHandle =
    aviutl2::generic::GlobalEditHandle::new();

enum CycleState {
    None,
    WaitingExpectedEdit {
        objects: HashMap<aviutl2::generic::ObjectHandle, Option<aviutl2::alias::Table>>,
    },
    PossiblyNextEdit {
        objects: HashMap<aviutl2::generic::ObjectHandle, Option<aviutl2::alias::Table>>,
    },
}

static CYCLE_STATE: std::sync::Mutex<CycleState> = std::sync::Mutex::new(CycleState::None);
static SETTINGS: std::sync::OnceLock<std::sync::Arc<std::sync::RwLock<config::Settings>>> =
    std::sync::OnceLock::new();
static EFFECTS: std::sync::LazyLock<std::collections::HashMap<String, aviutl2::generic::Effect>> =
    std::sync::LazyLock::new(|| {
        let mut effects = std::collections::HashMap::new();
        for effect in EDIT_HANDLE.get_effects() {
            effects.insert(effect.name.clone(), effect);
        }
        effects
    });

#[aviutl2::plugin(GenericPlugin)]
struct CreateControlObjectAux2;

static CONTROL_OBJECTS: &[&str] = &[
    "グループ制御",
    "グループ制御(音声)",
    "カメラ制御",
    "時間制御(オブジェクト)",
    "画像合成(オブジェクト)",
];

fn get_compatible_flags(effect_name: &str) -> Option<aviutl2::generic::EffectFlag> {
    match effect_name {
        "グループ制御" => Some(aviutl2::bitflag!(aviutl2::generic::EffectFlag {
            video: true,
            audio: false,
            as_filter: false,
            camera: false,
        })),
        "グループ制御(音声)" => Some(aviutl2::bitflag!(aviutl2::generic::EffectFlag {
            video: false,
            audio: true,
            as_filter: false,
            camera: false,
        })),
        "カメラ制御" => Some(aviutl2::bitflag!(aviutl2::generic::EffectFlag {
            video: false,
            audio: false,
            as_filter: false,
            camera: true,
        })),
        "時間制御(オブジェクト)" => {
            Some(aviutl2::bitflag!(aviutl2::generic::EffectFlag {
                video: false,
                audio: false,
                as_filter: false,
                camera: false,
            }))
        }
        "画像合成(オブジェクト)" => {
            Some(aviutl2::bitflag!(aviutl2::generic::EffectFlag {
                video: true,
                audio: false,
                as_filter: true,
                camera: false,
            }))
        }
        _ => None,
    }
}

impl aviutl2::generic::GenericPlugin for CreateControlObjectAux2 {
    fn new(_info: aviutl2::common::AviUtl2Info) -> aviutl2::common::AnyResult<Self> {
        aviutl2::tracing_subscriber::fmt()
            .with_max_level(if cfg!(debug_assertions) {
                aviutl2::tracing::Level::DEBUG
            } else {
                aviutl2::tracing::Level::INFO
            })
            .event_format(aviutl2::logger::AviUtl2Formatter)
            .with_writer(aviutl2::logger::AviUtl2LogWriter)
            .init();

        let settings = config::Settings::load()?;
        SETTINGS
            .set(std::sync::Arc::new(std::sync::RwLock::new(settings)))
            .map_err(|_| anyhow::anyhow!("Settings must only be initialized once."))?;
        Ok(Self)
    }

    fn plugin_info(&self) -> aviutl2::generic::GenericPluginTable {
        aviutl2::generic::GenericPluginTable {
            name: "create_control_object.aux2".to_string(),
            information: format!(
                "Create Control Object for Selected Objects / v{} / https://github.com/sevenc-nanashi/create_control_object.aux2",
                env!("CARGO_PKG_VERSION")
            ),
        }
    }

    fn register(&mut self, registry: &mut aviutl2::generic::HostAppHandle) {
        EDIT_HANDLE.init(registry.create_edit_handle());
        registry.register_menus::<Self>();
        for &control_object in CONTROL_OBJECTS {
            let create_from_selected = format!(
                "create_control_object.aux2\\選択オブジェクトから{}を作成",
                control_object
            );
            registry.register_object_menu(&create_from_selected, move || {
                play_beep_if_error(&CreateControlObjectAux2::create_control(control_object));
            });
            registry.register_edit_menu(&create_from_selected, move || {
                play_beep_if_error(&CreateControlObjectAux2::create_control(control_object));
            });
        }
        for &control_object in CONTROL_OBJECTS {
            let selected_into = format!(
                "create_control_object.aux2\\選択している制御オブジェクトを{}に変換",
                control_object
            );
            registry.register_object_menu(&selected_into, move || {
                play_beep_if_error(&CreateControlObjectAux2::select_into_control(
                    control_object,
                ));
            });
            registry.register_edit_menu(&selected_into, move || {
                play_beep_if_error(&CreateControlObjectAux2::select_into_control(
                    control_object,
                ));
            });
            registry.register_object_item_and_effect_menu(&selected_into, move |_, _, _, _| {
                play_beep_if_error(&CreateControlObjectAux2::select_into_control(
                    control_object,
                ));
            });
        }
        let cycle_control_object_type =
            "create_control_object.aux2\\選択している制御オブジェクトの種別を変更";
        registry.register_object_menu(cycle_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(1));
        });
        registry.register_edit_menu(cycle_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(1));
        });
        let reverse_control_object_type =
            "create_control_object.aux2\\選択している制御オブジェクトの種別を変更（逆順）";
        registry.register_object_menu(reverse_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(-1));
        });
        registry.register_edit_menu(reverse_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(-1));
        });
    }
    fn event_update_object_info(&mut self) {
        let mut state = CYCLE_STATE.lock().unwrap();
        match &*state {
            CycleState::WaitingExpectedEdit { objects } => {
                tracing::debug!("WaitingExpectedEdit -> PossiblyNextEdit");
                *state = CycleState::PossiblyNextEdit {
                    objects: objects.clone(),
                };
            }
            CycleState::PossiblyNextEdit { .. } => {
                tracing::debug!("PossiblyNextEdit -> None");
                *state = CycleState::None;
            }
            _ => {}
        }
    }
}

#[aviutl2::generic::menus]
impl CreateControlObjectAux2 {
    #[config(name = "create_control_object.aux2 設定")]
    fn show_config(parent: aviutl2::Win32WindowHandle) -> aviutl2::common::AnyResult<()> {
        let settings = SETTINGS
            .get()
            .expect("Settings must be initialized before opening the config dialog.")
            .clone();
        config_dialog::show(parent, settings)
    }
}

fn play_beep_if_error(result: &aviutl2::common::AnyResult<()>) {
    if let Err(err) = result {
        aviutl2::lprintln!(error, "{:?}", err);
        play_beep();
    }
}

fn play_beep() {
    unsafe {
        let _ = windows::Win32::System::Diagnostics::Debug::MessageBeep(
            windows::Win32::UI::WindowsAndMessaging::MB_ICONEXCLAMATION,
        );
    }
}

impl CreateControlObjectAux2 {
    fn create_control(effect_name: &str) -> aviutl2::common::AnyResult<()> {
        EDIT_HANDLE.call_edit_section(|e| {
            let mut selected_objects = e.get_selected_objects()?;
            if selected_objects.is_empty() {
                if let Some(focused_object) = e.get_focused_object()? {
                    selected_objects.push(focused_object);
                } else {
                    anyhow::bail!("No objects selected.");
                }
            }

            let positions = selected_objects
                .iter()
                .map(|obj| e.get_object_layer_frame(*obj))
                .collect::<Result<Vec<_>, _>>()?;

            let min_frame = positions.iter().map(|pos| pos.start).min().unwrap();
            let max_frame = positions.iter().map(|pos| pos.end).max().unwrap();
            let min_layer = positions.iter().map(|pos| pos.layer).min().unwrap();
            let max_layer = positions.iter().map(|pos| pos.layer).max().unwrap();

            if min_layer == 0 {
                anyhow::bail!("Cannot create group control at layer 0.");
            }

            let group_object = e.create_object(
                effect_name,
                min_layer - 1,
                min_frame,
                Some(max_frame - min_frame + 1),
            )?;

            e.set_object_effect_item(
                group_object,
                effect_name,
                0,
                "対象レイヤー数",
                &(max_layer - min_layer + 1).to_string(),
            )?;

            e.set_focus_object(Some(group_object))?;

            let mut state = CYCLE_STATE.lock().unwrap();
            *state = CycleState::WaitingExpectedEdit {
                objects: HashMap::from_iter([(group_object, None)]),
            };

            anyhow::Ok(())
        })?
    }

    fn select_into_control(effect_name: &str) -> aviutl2::common::AnyResult<()> {
        EDIT_HANDLE.call_edit_section(|e| {
            let mut selected_objects = e.get_selected_objects()?;
            let focused_object = e.get_focused_object()?;
            if selected_objects.is_empty() {
                if let Some(focused_object) = focused_object {
                    selected_objects.push(focused_object);
                } else {
                    anyhow::bail!("No objects selected.");
                }
            }

            let mut object_states = HashMap::new();
            for obj in selected_objects {
                let front_effect = e.get_effect_name(e.get_first_effect(obj)?)?;
                if CONTROL_OBJECTS.contains(&front_effect.as_str()) && front_effect != effect_name {
                    let is_focused = Some(obj) == focused_object;
                    let (new_handle, original_table) =
                        Self::convert_control_object_type(e, obj, effect_name)?;
                    if is_focused {
                        e.set_focus_object(Some(new_handle))?;
                    }
                    object_states.insert(new_handle, original_table);
                }
            }

            if object_states.is_empty() {
                anyhow::bail!("No control objects were converted.");
            }

            let mut state = CYCLE_STATE.lock().unwrap();
            *state = CycleState::WaitingExpectedEdit {
                objects: object_states,
            };

            anyhow::Ok(())
        })?
    }

    fn cycle_control_object_type(direction: i32) -> aviutl2::common::AnyResult<()> {
        EDIT_HANDLE.call_edit_section(|e| {
            let mut selected_objects = e.get_selected_objects()?;
            let focused_object = e.get_focused_object()?;
            if selected_objects.is_empty() {
                if let Some(focused_object) = focused_object {
                    selected_objects.push(focused_object);
                } else {
                    anyhow::bail!("No objects selected.");
                }
            }

            let mut object_states = HashMap::new();
            for obj in selected_objects {
                let front_effect = e.get_effect_name(e.get_first_effect(obj)?)?;
                if CONTROL_OBJECTS.contains(&front_effect.as_str()) {
                    let is_focused = Some(obj) == focused_object;
                    let index = CONTROL_OBJECTS
                        .iter()
                        .position(|&name| name == front_effect)
                        .unwrap();
                    let mut candidates = CONTROL_OBJECTS.to_vec();
                    candidates.rotate_left(index + 1);
                    if direction < 0 {
                        candidates.reverse();
                    }
                    candidates.pop();

                    if SETTINGS
                        .get()
                        .unwrap()
                        .read()
                        .unwrap()
                        .incompatible_effect_handling
                        == crate::config::IncompatibleEffectHandling::RemoveIncompatibleEffects
                    {
                        candidates = vec![candidates.first().unwrap()];
                    }

                    tracing::debug!("Cycling control object type for object {:?}", obj,);

                    let mut success = false;
                    for &new_effect_name in &candidates {
                        let (new_handle, original_table) =
                            match Self::convert_control_object_type(e, obj, new_effect_name) {
                                Ok(result) => result,
                                Err(err) => {
                                    tracing::warn!(
                                        "Failed to convert object {:?} to {}: {:?}",
                                        obj,
                                        new_effect_name,
                                        err
                                    );
                                    continue;
                                }
                            };
                        if is_focused {
                            e.set_focus_object(Some(new_handle))?;
                        }
                        object_states.insert(new_handle, original_table);
                        success = true;
                        break;
                    }

                    if !success {
                        anyhow::bail!(
                            "Failed to convert object {:?} to any compatible control object type.",
                            obj
                        );
                    }
                }
            }

            if object_states.is_empty() {
                anyhow::bail!("No control objects were converted.");
            }

            let mut state = CYCLE_STATE.lock().unwrap();
            *state = CycleState::WaitingExpectedEdit {
                objects: object_states,
            };

            anyhow::Ok(())
        })?
    }

    fn convert_control_object_type(
        edit: &mut aviutl2::generic::EditSection,
        obj: aviutl2::generic::ObjectHandle,
        new_effect_name: &str,
    ) -> aviutl2::common::AnyResult<(
        aviutl2::generic::ObjectHandle,
        Option<aviutl2::alias::Table>,
    )> {
        let original_alias = {
            let state = CYCLE_STATE.lock().unwrap();
            if let CycleState::PossiblyNextEdit { objects } = &*state
                && let Some(alias) = objects.get(&obj)
            {
                tracing::debug!(
                    "Continuing with previously stored alias for object {:?}",
                    obj
                );
                alias.clone()
            } else {
                Some(edit.get_object_alias_parsed(obj)?)
            }
        };

        let position = edit.get_object_layer_frame(obj)?;
        let new_object = if let Some(mut alias) = original_alias.clone() {
            tracing::debug!(
                "Converting object {:?} to {} using alias",
                obj,
                new_effect_name
            );
            let mut objects = (0..)
                .map_while(|i| {
                    let table_name = format!("Object.{}", i);
                    if let Some(table) = alias.get_table(&table_name).cloned() {
                        alias.remove_table(&table_name);
                        Some(table)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let old_effect_name = objects[0]
                .get_value("effect.name")
                .ok_or_else(|| anyhow::anyhow!("effect.name not found in Object.0"))?
                .to_owned();
            if old_effect_name == "画像合成(オブジェクト)"
                && new_effect_name != "画像合成(オブジェクト)"
            {
                // 画像合成(オブジェクト)は出力エフェクトがあるので、そこからパラメーターをObject.0に引き継ぐ
                let output_effect_table_index = objects
                    .iter()
                    .position(|table| {
                        table
                            .get_value("effect.name")
                            .map(|name| {
                                EFFECTS.get(name).unwrap().effect_type
                                    == aviutl2::generic::EffectType::Output
                            })
                            .unwrap_or(false)
                    })
                    .ok_or_else(|| anyhow::anyhow!("Output effect table not found"))?;
                let output_effect_table = objects.remove(output_effect_table_index);
                let object_0 = objects.get_mut(0).unwrap();
                for (key, value) in output_effect_table.values() {
                    if key != "effect.name" {
                        object_0.insert_value(key, value);
                    }
                }
                object_0.insert_value("effect.name", new_effect_name);
            } else if old_effect_name != "画像合成(オブジェクト)"
                && new_effect_name == "画像合成(オブジェクト)"
            {
                // 画像合成(オブジェクト)は出力エフェクトがあるので、それに座標とかを引き継ぐ
                let object_0 = objects.get_mut(0).unwrap();
                object_0.insert_value("effect.name", new_effect_name);
                let mut new_object_0 = object_0.clone();
                new_object_0.insert_value("effect.name", "標準描画");
                objects.push(new_object_0);
            } else {
                let object_0 = objects.get_mut(0).unwrap();
                object_0.insert_value("effect.name", new_effect_name);
            }

            if SETTINGS
                .get()
                .unwrap()
                .read()
                .unwrap()
                .incompatible_effect_handling
                == crate::config::IncompatibleEffectHandling::RemoveIncompatibleEffects
            {
                let compatible_flags = get_compatible_flags(new_effect_name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown effect name: {}", new_effect_name))?;
                objects.retain(|table| {
                    let effect_name = table.get_value("effect.name");
                    if effect_name.map(|n| n.as_str()) == Some(new_effect_name) {
                        return true;
                    }
                    if let Some(effect_name) = effect_name
                        && let Some(effect) = EFFECTS.get(effect_name)
                    {
                        let mut effect_flag = effect.flag;
                        effect_flag.as_filter = false;

                        if compatible_flags.to_bits() & effect_flag.to_bits() == 0 {
                            tracing::debug!(
                                "Removing incompatible effect {} for new effect {}",
                                effect_name,
                                new_effect_name
                            );
                            return false;
                        }
                    }
                    true
                });
            }

            for (i, table) in objects.into_iter().enumerate() {
                let table_name = format!("Object.{}", i);
                alias.insert_table(&table_name, table);
            }

            // edit.move_object(obj, edit.info.layer_max + 1, 0)?;
            edit.create_object_from_alias(
                &alias.to_string(),
                edit.info.layer_max + 1,
                position.start,
                position.end - position.start + 1,
            )
        } else {
            tracing::debug!(
                "Recreating object {:?} as {} without alias",
                obj,
                new_effect_name
            );
            let num_layers = CONTROL_OBJECTS
                .iter()
                .find_map(|&name| {
                    edit.get_object_effect_item(obj, name, 0, "対象レイヤー数")
                        .ok()
                        .and_then(|o| o.parse::<usize>().ok())
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Failed to get the number of target layers for object {:?}",
                        obj
                    )
                })?;
            edit.create_object(
                new_effect_name,
                edit.info.layer_max + 1,
                position.start,
                Some(position.end - position.start + 1),
            )
            .and_then(|new_obj| {
                edit.set_object_effect_item(
                    new_obj,
                    new_effect_name,
                    0,
                    "対象レイヤー数",
                    &num_layers.to_string(),
                )?;
                Ok(new_obj)
            })
        };

        match new_object {
            Ok(new_obj) => {
                edit.delete_object(obj)?;
                edit.move_object(new_obj, position.layer, position.start)?;
                Ok((new_obj, original_alias))
            }
            Err(err) => Err(err.into()),
        }
    }
}

aviutl2::register_generic_plugin!(CreateControlObjectAux2);
