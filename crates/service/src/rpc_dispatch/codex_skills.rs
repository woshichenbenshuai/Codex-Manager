use codexmanager_core::rpc::types::{JsonRpcRequest, JsonRpcResponse};

pub(super) fn try_handle(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let result = match req.method.as_str() {
        "codexSkills/list" => super::value_or_error(crate::codex_skills::list(super::str_param(
            req,
            "codexHome",
        ))),
        "codexSkills/installZip" => super::value_or_error(crate::codex_skills::install_zip(
            super::str_param(req, "fileName"),
            super::str_param(req, "archiveBase64"),
            super::str_param(req, "codexHome"),
        )),
        "codexSkills/importDirectory" => {
            super::value_or_error(crate::codex_skills::import_directory(
                super::str_param(req, "sourcePath"),
                super::str_param(req, "codexHome"),
            ))
        }
        "codexSkills/delete" => super::value_or_error(crate::codex_skills::delete(
            super::str_param(req, "directoryName"),
            super::str_param(req, "codexHome"),
        )),
        "codexSkills/marketplaceList" => super::value_or_error(
            crate::codex_skills_marketplace::list(super::str_param(req, "codexHome")),
        ),
        "codexSkills/marketplaceAdd" => {
            super::value_or_error(crate::codex_skills_marketplace::add(
                super::str_param(req, "source"),
                super::str_param(req, "refName"),
                super::str_param(req, "codexHome"),
            ))
        }
        "codexSkills/marketplaceRefresh" => {
            super::value_or_error(crate::codex_skills_marketplace::refresh(
                super::str_param(req, "marketplaceName"),
                super::str_param(req, "codexHome"),
            ))
        }
        "codexSkills/marketplacePluginInstall" => {
            super::value_or_error(crate::codex_skills_marketplace::install(
                super::str_param(req, "pluginId"),
                super::str_param(req, "codexHome"),
            ))
        }
        _ => return None,
    };

    Some(super::response(req, result))
}
