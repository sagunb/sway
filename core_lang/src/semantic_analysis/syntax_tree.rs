use super::{TypedAstNode, TypedAstNodeContent, TypedDeclaration, TypedFunctionDeclaration};
use crate::build_config::BuildConfig;
use crate::control_flow_analysis::ControlFlowGraph;
use crate::semantic_analysis::Namespace;
use crate::ParseTree;
use crate::{
    error::*,
    types::{MaybeResolvedType, ResolvedType},
};
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeType {
    Predicate,
    Script,
    Contract,
    Library,
}

#[derive(Debug)]
pub(crate) enum TypedParseTree<'sc> {
    Script {
        main_function: TypedFunctionDeclaration<'sc>,
        namespace: Namespace<'sc>,
        declarations: Vec<TypedDeclaration<'sc>>,
        all_nodes: Vec<TypedAstNode<'sc>>,
    },
    Predicate {
        main_function: TypedFunctionDeclaration<'sc>,
        namespace: Namespace<'sc>,
        declarations: Vec<TypedDeclaration<'sc>>,
        all_nodes: Vec<TypedAstNode<'sc>>,
    },
    Contract {
        abi_entries: Vec<TypedFunctionDeclaration<'sc>>,
        namespace: Namespace<'sc>,
        declarations: Vec<TypedDeclaration<'sc>>,
        all_nodes: Vec<TypedAstNode<'sc>>,
    },
    Library {
        namespace: Namespace<'sc>,
        all_nodes: Vec<TypedAstNode<'sc>>,
    },
}

impl<'sc> TypedParseTree<'sc> {
    /// The `all_nodes` field in the AST variants is used to perform control flow and return flow
    /// analysis, while the direct copies of the declarations and main functions are used to create
    /// the ASM.
    pub(crate) fn all_nodes(&self) -> &[TypedAstNode<'sc>] {
        use TypedParseTree::*;
        match self {
            Library { all_nodes, .. } => all_nodes,
            Script { all_nodes, .. } => all_nodes,
            Contract { all_nodes, .. } => all_nodes,
            Predicate { all_nodes, .. } => all_nodes,
        }
    }
    pub(crate) fn namespace(&self) -> &Namespace<'sc> {
        use TypedParseTree::*;
        match self {
            Library { namespace, .. } => namespace,
            Script { namespace, .. } => namespace,
            Contract { namespace, .. } => namespace,
            Predicate { namespace, .. } => namespace,
        }
    }
    pub(crate) fn type_check(
        parsed: ParseTree<'sc>,
        initial_namespace: Namespace<'sc>,
        tree_type: TreeType,
        build_config: &BuildConfig,
        dead_code_graph: &mut ControlFlowGraph<'sc>,
    ) -> CompileResult<'sc, Self> {
        let mut initial_namespace = initial_namespace.clone();
        let mut successful_nodes = vec![];
        let mut next_pass_nodes: VecDeque<_> = parsed.root_nodes.into_iter().collect();
        let mut num_failed_nodes = next_pass_nodes.len();
        let mut warnings = Vec::new();
        let mut is_first_pass = true;
        let mut errors = Vec::new();
        while num_failed_nodes > 0 {
            let nodes = next_pass_nodes
                .clone()
                .into_iter()
                .map(|node| {
                    (
                        node.clone(),
                        TypedAstNode::type_check(
                            node,
                            &mut initial_namespace,
                            None,
                            "",
                            // TODO only allow impl traits on contract trees, do something else
                            // for other tree types
                            &MaybeResolvedType::Resolved(ResolvedType::Contract),
                            build_config,
                            dead_code_graph,
                        ),
                    )
                })
                .collect::<Vec<(_, CompileResult<_>)>>();
            next_pass_nodes = Default::default();

            // If we hit the internal "non-decreasing error nodes" error, this helps
            // show what went wrong right beforehand.
            let mut errors_from_this_pass = vec![];
            for (node, mut res) in nodes.clone() {
                if res.value.is_none() {
                    errors_from_this_pass.append(&mut res.errors);
                    next_pass_nodes.push_front(node);
                } else {
                    if res.errors.is_empty() {
                        successful_nodes.push(res);
                    } else {
                        errors_from_this_pass.append(&mut res.errors);
                        next_pass_nodes.push_front(node);
                    }
                }
            }
            // If we did not solve any issues, i.e. the same number of nodes failed,
            // then this is a genuine error and so we break.
            if next_pass_nodes.len() == num_failed_nodes && !is_first_pass {
                for (_, mut failed_node_res) in nodes {
                    warnings.append(&mut failed_node_res.warnings);
                    errors.append(&mut failed_node_res.errors);
                }
                break;
            }
            is_first_pass = false;
            // if the amount of nodes with errors is going up, then bail.
            if next_pass_nodes.len() > num_failed_nodes {
                errors.append(&mut errors_from_this_pass);
                return err(warnings, errors);
            }
            num_failed_nodes = next_pass_nodes.len();
        }

        let typed_tree_nodes = successful_nodes
            .into_iter()
            .filter_map(|res| res.ok(&mut warnings, &mut errors))
            .collect::<Vec<TypedAstNode<'sc>>>();

        // perform validation based on the tree type
        match tree_type {
            TreeType::Predicate => {
                // a predicate must have a main function and that function must return a boolean
                let all_nodes = typed_tree_nodes.clone();
                let main_func_vec = typed_tree_nodes
                    .iter()
                    .filter_map(|TypedAstNode { content, .. }| match content {
                        TypedAstNodeContent::Declaration(
                            TypedDeclaration::FunctionDeclaration(func),
                        ) => {
                            if func.name.primary_name == "main" {
                                Some(func)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                if main_func_vec.len() > 1 {
                    errors.push(CompileError::MultiplePredicateMainFunctions(
                        main_func_vec.last().unwrap().span.clone(),
                    ));
                } else if main_func_vec.is_empty() {
                    errors.push(CompileError::NoPredicateMainFunction(parsed.span));
                    return err(warnings, errors);
                }
                let main_func = main_func_vec[0];
                match main_func.return_type {
                    MaybeResolvedType::Resolved(ResolvedType::Boolean) => (),
                    _ => errors.push(CompileError::PredicateMainDoesNotReturnBool(
                        main_func.span.clone(),
                    )),
                }
                ok(
                    TypedParseTree::Predicate {
                        main_function: main_func.clone(),
                        all_nodes,
                        namespace: initial_namespace,
                        declarations: typed_tree_nodes
                            .into_iter()
                            .filter_map(|TypedAstNode { content, .. }| match content {
                                TypedAstNodeContent::Declaration(a) => Some(a),
                                _ => None,
                            })
                            .collect(),
                    },
                    warnings,
                    errors,
                )
            }
            TreeType::Script => {
                // a script must have exactly one main function
                let all_nodes = typed_tree_nodes.clone();
                let main_func_vec = typed_tree_nodes
                    .iter()
                    .filter_map(|TypedAstNode { content, .. }| match content {
                        TypedAstNodeContent::Declaration(
                            TypedDeclaration::FunctionDeclaration(func),
                        ) => {
                            if func.name.primary_name == "main" {
                                Some(func)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                if main_func_vec.len() > 1 {
                    errors.push(CompileError::MultipleScriptMainFunctions(
                        main_func_vec.last().unwrap().span.clone(),
                    ));
                } else if main_func_vec.is_empty() {
                    errors.push(CompileError::NoScriptMainFunction(parsed.span));
                    return err(warnings, errors);
                }

                let main_func = main_func_vec[0];

                ok(
                    TypedParseTree::Script {
                        main_function: main_func.clone(),
                        namespace: initial_namespace,
                        all_nodes,
                        declarations: typed_tree_nodes
                            .into_iter()
                            .filter_map(|TypedAstNode { content, .. }| match content {
                                TypedAstNodeContent::Declaration(a) => Some(a),
                                _ => None,
                            })
                            .collect(),
                    },
                    warnings,
                    errors,
                )
            }
            TreeType::Library => ok(
                TypedParseTree::Library {
                    all_nodes: typed_tree_nodes,
                    namespace: initial_namespace,
                },
                warnings,
                errors,
            ),
            TreeType::Contract => {
                // abi entries are all functions declared in impl_traits
                // on the contract type itself
                let mut abi_entries = vec![];
                let mut declarations = vec![];
                let all_nodes = typed_tree_nodes.clone();
                for node in typed_tree_nodes {
                    match node {
                        TypedAstNode {
                            content:
                                TypedAstNodeContent::Declaration(TypedDeclaration::ImplTrait {
                                    methods,
                                    type_implementing_for:
                                        MaybeResolvedType::Resolved(ResolvedType::Contract),
                                    ..
                                }),
                            ..
                        } => abi_entries.append(&mut methods.clone()),
                        TypedAstNode {
                            content: TypedAstNodeContent::Declaration(a),
                            ..
                        } => declarations.push(a),
                        _ => (),
                    }
                }
                ok(
                    TypedParseTree::Contract {
                        abi_entries,
                        namespace: initial_namespace,
                        declarations,
                        all_nodes,
                    },
                    warnings,
                    errors,
                )
            }
        }
        /*
        ok(
            TypedParseTree {
                root_nodes: typed_tree_nodes,
                namespace: initial_namespace,
            },
            warnings,
            errors,
        )*/
    }
}