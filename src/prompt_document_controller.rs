use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use esbuild_metafile::EsbuildMetaFile;
use markdown::mdast::Node;

use crate::asset_manager::AssetManager;
use crate::asset_path_renderer::AssetPathRenderer;
use crate::content_document_linker::ContentDocumentLinker;
use crate::eval_prompt_document_mdast::eval_prompt_document_mdast;
use crate::eval_prompt_document_mdast_params::EvalPromptDocumentMdastParams;
use crate::mcp::jsonrpc::request::prompts_get::PromptsGet;
use crate::mcp::jsonrpc::request::prompts_get::PromptsGetParams;
use crate::mcp::jsonrpc::response::success::prompts_get_result::PromptsGetResult;
use crate::mcp::prompt::Prompt;
use crate::mcp::prompt::PromptArgument;
use crate::mcp::prompt_controller::PromptController;
use crate::prompt_document_component_context::PromptDocumentComponentContext;
use crate::prompt_document_front_matter::PromptDocumentFrontMatter;
use crate::prompt_document_front_matter::argument::Argument;
use crate::rhai_template_renderer::RhaiTemplateRenderer;

pub struct PromptDocumentController {
    pub asset_path_renderer: AssetPathRenderer,
    pub content_document_linker: ContentDocumentLinker,
    pub esbuild_metafile: Arc<EsbuildMetaFile>,
    pub front_matter: PromptDocumentFrontMatter,
    pub name: String,
    pub mdast: Node,
    pub rhai_template_renderer: RhaiTemplateRenderer,
}

#[async_trait]
impl PromptController for PromptDocumentController {
    fn get_mcp_prompt(&self) -> Prompt {
        Prompt {
            arguments: self
                .front_matter
                .clone()
                .arguments
                .into_iter()
                .map(
                    |(
                        name,
                        Argument {
                            date,
                            description,
                            required,
                            title,
                        },
                    )| PromptArgument {
                        date,
                        description,
                        name,
                        required,
                        title,
                    },
                )
                .collect(),
            description: self.front_matter.description.clone(),
            name: self.name.clone(),
            title: self.front_matter.title.clone(),
        }
    }

    async fn respond_to(
        &self,
        PromptsGet {
            params: PromptsGetParams { arguments, .. },
            ..
        }: PromptsGet,
    ) -> Result<PromptsGetResult> {
        let mut prompt_document_component_context = PromptDocumentComponentContext {
            arguments: self.front_matter.map_arguments(arguments)?,
            asset_manager: AssetManager::from_esbuild_metafile(
                self.esbuild_metafile.clone(),
                self.asset_path_renderer.clone(),
            ),
            content_document_linker: self.content_document_linker.clone(),
            current_role: Default::default(),
            front_matter: self.front_matter.clone(),
            prompt_messages: Default::default(),
            unprocessed_message_chunk: Default::default(),
        };

        eval_prompt_document_mdast(
            EvalPromptDocumentMdastParams {
                mdast: &self.mdast,
                is_directly_in_root: false,
                is_first_child: false,
                is_in_top_paragraph: false,
                rhai_template_renderer: &self.rhai_template_renderer,
            },
            &mut prompt_document_component_context,
        )?;

        Ok(PromptsGetResult {
            description: Some(self.front_matter.description.clone()),
            messages: prompt_document_component_context.prompt_messages,
            meta: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use indoc::indoc;

    use super::*;
    use crate::build_prompt_document_controller::build_prompt_document_controller;
    use crate::build_prompt_document_controller_params::BuildPromptDocumentControllerParams;
    use crate::filesystem::file_entry_stub::FileEntryStub;
    use crate::mcp::jsonrpc::JSONRPC_VERSION;
    use crate::mcp::jsonrpc::role::Role;
    use crate::mcp::prompt_message::PromptMessage;
    use crate::rhai_template_factory::RhaiTemplateFactory;

    #[tokio::test]
    async fn test_convert_to_prompt_messages() -> Result<()> {
        let name: String = "help-me-finish-task".to_string();
        let contents: String = indoc! {r#"
        +++
        description = "test prompt description"
        title = "Help me with finishing the task"
        date = "31/10/2024"

        [arguments.objective]
        description = "Describe what you are trying to do"
        required = true
        title = "Your objective"
        +++

        **user**: This is what I am trying to do: {context.arguments.objective.input}

        **assistant**: wow

        **user**: yeah
        "#}
        .to_string();

        let rhai_template_factory = RhaiTemplateFactory::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            PathBuf::from("shortcodes"),
        );

        let rhai_template_renderer: RhaiTemplateRenderer = rhai_template_factory.try_into()?;

        let prompt_controller =
            build_prompt_document_controller(BuildPromptDocumentControllerParams {
                asset_path_renderer: AssetPathRenderer {
                    base_path: "https://example.com".to_string(),
                },
                content_document_linker: Default::default(),
                esbuild_metafile: Default::default(),
                file: FileEntryStub {
                    contents,
                    relative_path: PathBuf::from("prompts/help-me-finish-task.md"),
                }
                .try_into()?,
                name: name.clone(),
                rhai_template_renderer,
            })?;

        let response = prompt_controller
            .respond_to(PromptsGet {
                id: "1".into(),
                jsonrpc: JSONRPC_VERSION.to_string(),
                params: PromptsGetParams {
                    arguments: {
                        let mut arguments: HashMap<String, String> = Default::default();

                        arguments.insert("objective".to_string(), "ride a horse".to_string());

                        arguments
                    },
                    meta: None,
                    name,
                },
            })
            .await?;

        assert_eq!(
            response.description,
            Some("test prompt description".to_string())
        );
        assert_eq!(response.messages.len(), 3);

        let message_0: &PromptMessage = response.messages.first().unwrap();

        assert_eq!(message_0.role, Role::User);
        assert_eq!(
            message_0.content,
            "This is what I am trying to do: ride a horse".into()
        );

        let message_1: &PromptMessage = response.messages.get(1).unwrap();

        assert_eq!(message_1.role, Role::Assistant);
        assert_eq!(message_1.content, "wow".into());

        let message_2: &PromptMessage = response.messages.get(2).unwrap();

        assert_eq!(message_2.role, Role::User);
        assert_eq!(message_2.content, "yeah".into());

        Ok(())
    }
}
