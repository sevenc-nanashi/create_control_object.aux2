use aviutl2::anyhow;

pub static EDIT_HANDLE: aviutl2::generic::GlobalEditHandle =
    aviutl2::generic::GlobalEditHandle::new();

#[aviutl2::plugin(GenericPlugin)]
struct CreateControlObjectAux2;

static CONTROL_OBJECTS: &[&str] = &[
    "グループ制御",
    "グループ制御(音声)",
    "カメラ制御",
    "時間制御(オブジェクト)",
    "画像合成(オブジェクト)",
];

impl aviutl2::generic::GenericPlugin for CreateControlObjectAux2 {
    fn new(_info: aviutl2::common::AviUtl2Info) -> aviutl2::common::AnyResult<Self> {
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
        let cycle_control_object_type = "create_control_object.aux2\\制御オブジェクトの種別を変更";
        registry.register_object_menu(cycle_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(1));
        });
        registry.register_edit_menu(cycle_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(1));
        });
        let reverse_control_object_type =
            "create_control_object.aux2\\制御オブジェクトの種別を変更（逆順）";
        registry.register_object_menu(reverse_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(-1));
        });
        registry.register_edit_menu(reverse_control_object_type, || {
            play_beep_if_error(&CreateControlObjectAux2::cycle_control_object_type(-1));
        });
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
            let selected_objects = e.get_selected_objects()?;
            if selected_objects.is_empty() {
                anyhow::bail!("No objects selected.");
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

            let mut success_count = 0;
            for obj in selected_objects {
                let front_effect = e.get_effect_name(e.get_first_effect(obj)?)?;
                if CONTROL_OBJECTS.contains(&front_effect.as_str()) && front_effect != effect_name {
                    let is_focused = Some(obj) == focused_object;
                    let new_handle = Self::convert_control_object_type(e, obj, effect_name)?;
                    if is_focused {
                        e.set_focus_object(Some(new_handle))?;
                    }
                    success_count += 1;
                }
            }

            if success_count == 0 {
                anyhow::bail!("No control objects were converted.");
            }

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

            let mut success_count = 0;
            for obj in selected_objects {
                let front_effect = e.get_effect_name(e.get_first_effect(obj)?)?;
                if CONTROL_OBJECTS.contains(&front_effect.as_str()) {
                    let current_index = CONTROL_OBJECTS
                        .iter()
                        .position(|&name| name == front_effect)
                        .unwrap();
                    let new_index = (current_index as i32 + direction)
                        .rem_euclid(CONTROL_OBJECTS.len() as i32)
                        as usize;
                    let new_effect_name = CONTROL_OBJECTS[new_index];
                    let is_focused = Some(obj) == focused_object;
                    let new_handle = Self::convert_control_object_type(e, obj, new_effect_name)?;
                    if is_focused {
                        e.set_focus_object(Some(new_handle))?;
                    }
                    success_count += 1;
                }
            }

            if success_count == 0 {
                anyhow::bail!("No control objects were converted.");
            }

            anyhow::Ok(())
        })?
    }

    fn convert_control_object_type(
        edit: &mut aviutl2::generic::EditSection,
        obj: aviutl2::generic::ObjectHandle,
        new_effect_name: &str,
    ) -> aviutl2::common::AnyResult<aviutl2::generic::ObjectHandle> {
        let original_alias = edit.get_object_alias_parsed(obj)?;
        let mut alias = original_alias.clone();
        let object_0 = alias
            .get_table_mut("Object.0")
            .ok_or_else(|| anyhow::anyhow!("Object.0 table not found"))?;
        let old_effect_name = object_0
            .get_value("effect.name")
            .ok_or_else(|| anyhow::anyhow!("effect.name not found in Object.0"))?
            .to_owned();
        object_0.insert_value("effect.name", &new_effect_name);
        if old_effect_name == "画像合成(オブジェクト)" {
            // 画像合成(オブジェクト)は出力エフェクトがあるので、それがなくなる分を詰める
            let object_1 = alias
                .get_table("Object.1")
                .ok_or_else(|| anyhow::anyhow!("Object.1 table not found"))?
                .clone();
            for i in 2.. {
                let table_name = format!("Object.{}", i);
                if let Some(table) = alias.get_table(&table_name) {
                    alias.insert_table(&format!("Object.{}", i - 1), table.clone());
                } else {
                    alias.remove_table(&format!("Object.{}", i - 1));
                    break;
                }
            }
            let object_0 = alias
                .get_table_mut("Object.0")
                .ok_or_else(|| anyhow::anyhow!("Object.0 table not found"))?;
            // 座標とかをグループ制御に引き継ぐ
            for (key, value) in object_1.values() {
                if key == "effect.name" {
                    continue;
                }
                object_0.insert_value(key, value);
            }
        } else if new_effect_name == "画像合成(オブジェクト)" {
            // 画像合成(オブジェクト)は出力エフェクトがあるので、それに座標とかを引き継ぐ
            let mut object_0 = alias
                .get_table("Object.0")
                .ok_or_else(|| anyhow::anyhow!("Object.0 table not found"))?
                .clone();
            object_0.insert_value("effect.name", "標準描画");
            let next_table = (0..)
                .find(|i| {
                    let table_name = format!("Object.{}", i);
                    alias.get_table(&table_name).is_none()
                })
                .expect("There should be a free table index");
            alias.insert_table(&format!("Object.{}", next_table), object_0);
        }

        let position = edit.get_object_layer_frame(obj)?;
        edit.move_object(obj, edit.info.layer_max + 1, 0)?;
        let new_object = edit.create_object_from_alias(
            &alias.to_string(),
            position.layer,
            position.start,
            position.end - position.start + 1,
        );
        match new_object {
            Ok(new_obj) => {
                edit.delete_object(obj)?;
                Ok(new_obj)
            }
            Err(err) => {
                edit.move_object(obj, position.layer, position.start)?;
                Err(err.into())
            }
        }
    }
}

aviutl2::register_generic_plugin!(CreateControlObjectAux2);
