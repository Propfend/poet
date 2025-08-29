use std::sync::Arc;

use rhai::Array;
use rhai::Dynamic;
use rhai::EvalAltResult;
use rhai::EvalContext;
use rhai::Map;

use super::attribute_value::AttributeValue;
use super::component_registry::ComponentRegistry;
use super::eval_tag::eval_tag;
use super::expression_collection::ExpressionCollection;
use super::tag_stack_node::TagStackNode;

pub fn eval_tag_stack_node(
    component_registry: Arc<ComponentRegistry>,
    eval_context: &mut EvalContext,
    current_node: &TagStackNode,
    expression_collection: &mut ExpressionCollection,
) -> Result<String, Box<EvalAltResult>> {
    match current_node {
        TagStackNode::BodyExpression(expression_reference) => {
            let body_expression_result =
                expression_collection.eval_expression(eval_context, expression_reference)?;

            if body_expression_result.is_array() {
                let body_expresion_array: Array = body_expression_result.as_array_ref()?.to_vec();
                let mut combined_ret = String::new();

                for item in body_expresion_array {
                    combined_ret.push_str(&item.to_string());
                }

                Ok(combined_ret)
            } else {
                Ok(body_expression_result.to_string())
            }
        }
        TagStackNode::Tag {
            children,
            is_closed,
            opening_tag,
        } => {
            let mut result = String::new();

            if let Some(opening_tag) = &opening_tag
                && !opening_tag.is_component()
            {
                result.push_str(&eval_tag(eval_context, expression_collection, opening_tag)?);
            }

            for child in children {
                result.push_str(&eval_tag_stack_node(
                    component_registry.clone(),
                    eval_context,
                    child,
                    expression_collection,
                )?);
            }

            if let Some(opening_tag) = &opening_tag
                && *is_closed
                && !opening_tag.is_component()
            {
                result.push_str(&format!("</{}>", opening_tag.name));

                return Ok(result);
            }

            if let Some(opening_tag) = &opening_tag
                && opening_tag.is_component()
            {
                let props = {
                    let mut props = Map::new();

                    for attribute in &opening_tag.attributes {
                        props.insert(
                            attribute.name.clone().into(),
                            if let Some(value) = &attribute.value {
                                match value {
                                    AttributeValue::Expression(expression_reference) => {
                                        expression_collection
                                            .eval_expression(eval_context, expression_reference)?
                                    }
                                    AttributeValue::Text(text) => text.into(),
                                }
                            } else {
                                true.into()
                            },
                        );
                    }

                    props
                };

                Ok(eval_context
                    .call_fn::<Dynamic>(
                        component_registry
                            .get_global_fn_name(&opening_tag.name)
                            .map_err(|err| {
                                EvalAltResult::ErrorRuntime(
                                    format!("Component not found: {err}").into(),
                                    rhai::Position::NONE,
                                )
                            })?,
                        (
                            match eval_context.scope().get("context") {
                                Some(context) => context.clone(),
                                None => {
                                    return Err(EvalAltResult::ErrorRuntime(
                                        "'context' variable not found in scope".into(),
                                        rhai::Position::NONE,
                                    )
                                    .into());
                                }
                            },
                            Dynamic::from_map(props),
                            Dynamic::from(result.clone()),
                        ),
                    )
                    .map_err(|err| {
                        EvalAltResult::ErrorRuntime(
                            format!("Failed to call component function: {err}").into(),
                            rhai::Position::NONE,
                        )
                    })?
                    .to_string())
            } else {
                Ok(result)
            }
        }
        TagStackNode::Text(text) => Ok(text.clone()),
    }
}
