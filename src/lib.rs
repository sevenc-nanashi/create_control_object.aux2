use aviutl2::anyhow;

pub static EDIT_HANDLE: aviutl2::generic::GlobalEditHandle =
    aviutl2::generic::GlobalEditHandle::new();

#[aviutl2::plugin(GenericPlugin)]
struct CreateControlObjectAux2;

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
        registry.register_menus::<Self>();
    }
}

impl CreateControlObjectAux2 {
    fn create_control(&mut self, effect_name: &str) -> aviutl2::common::AnyResult<()> {
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
}

#[aviutl2::generic::menus]
impl CreateControlObjectAux2 {
    #[edit(name = "create_control_object.aux2\\選択オブジェクトからグループ制御を作成")]
    #[object(name = "create_control_object.aux2\\選択オブジェクトからグループ制御を作成")]
    fn create_object_group(&mut self) -> aviutl2::common::AnyResult<()> {
        self.create_control("グループ制御")
    }

    #[edit(name = "create_control_object.aux2\\選択オブジェクトからグループ制御（音声）を作成")]
    #[object(name = "create_control_object.aux2\\選択オブジェクトからグループ制御（音声）を作成")]
    fn create_audio_group(&mut self) -> aviutl2::common::AnyResult<()> {
        self.create_control("グループ制御(音声)")
    }

    #[edit(name = "create_control_object.aux2\\選択オブジェクトから時間制御（オブジェクト）を作成")]
    #[object(name = "create_control_object.aux2\\選択オブジェクトから時間制御（オブジェクト）を作成")]
    fn create_timecontrol(&mut self) -> aviutl2::common::AnyResult<()> {
        self.create_control("時間制御(オブジェクト)")
    }

    #[edit(name = "create_control_object.aux2\\選択オブジェクトから画像合成（オブジェクト）を作成")]
    #[object(name = "create_control_object.aux2\\選択オブジェクトから画像合成（オブジェクト）を作成")]
    fn create_image_composition(&mut self) -> aviutl2::common::AnyResult<()> {
        self.create_control("画像合成(オブジェクト)")
    }
}

aviutl2::register_generic_plugin!(CreateControlObjectAux2);
