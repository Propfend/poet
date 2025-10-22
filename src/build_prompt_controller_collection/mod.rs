pub mod build_prompt_controller_collection_params;

use std::path::PathBuf;

use anyhow::Result;
use anyhow::anyhow;
use dashmap::DashMap;
use log::info;
use rayon::prelude::*;

use crate::build_prompt_controller::build_prompt_controller;
use crate::build_prompt_controller_collection::build_prompt_controller_collection_params::BuildPromptControllerCollectionParams;
use crate::build_prompt_controller_params::BuildPromptControllerParams;
use crate::build_timer::BuildTimer;
use crate::document_error_collection::DocumentErrorCollection;
use crate::filesystem::Filesystem as _;
use crate::prompt_controller::PromptController;
use crate::prompt_controller_collection::PromptControllerCollection;

pub async fn build_prompt_controller_collection(
    BuildPromptControllerCollectionParams {
        asset_path_renderer,
        content_document_linker,
        esbuild_metafile,
        rhai_template_renderer,
        source_filesystem,
    }: BuildPromptControllerCollectionParams,
) -> Result<PromptControllerCollection> {
    info!("Processing prompt files...");

    let _build_timer = BuildTimer::new();
    let error_collection: DocumentErrorCollection = Default::default();
    let prompt_controller_map: DashMap<String, PromptController> = Default::default();

    source_filesystem
        .read_project_files()
        .await?
        .into_par_iter()
        .filter(|file| file.kind.is_prompt())
        .for_each(|file| {
            let name = file
                .get_stem_path_relative_to(&PathBuf::from("prompts"))
                .display()
                .to_string();

            match build_prompt_controller(BuildPromptControllerParams {
                asset_path_renderer: asset_path_renderer.clone(),
                content_document_linker: content_document_linker.clone(),
                esbuild_metafile: esbuild_metafile.clone(),
                file,
                name: name.clone(),
                rhai_template_renderer: rhai_template_renderer.clone(),
            }) {
                Ok(prompt_controller) => {
                    prompt_controller_map.insert(name, prompt_controller);
                }
                Err(err) => {
                    error_collection.register_error(name, err);
                }
            }
        });

    if !error_collection.is_empty() {
        return Err(anyhow!("{error_collection}"));
    }

    Ok(prompt_controller_map.into())
}
