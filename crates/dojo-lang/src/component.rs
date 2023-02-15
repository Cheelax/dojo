use std::collections::HashMap;

use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_semantic::patcher::{ModifiedNode, PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use indoc::formatdoc;
use smol_str::SmolStr;

use crate::plugin::DojoAuxData;

pub struct Component {
    pub name: SmolStr,
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl Component {
    pub fn from_module_body(db: &dyn SyntaxGroup, name: SmolStr, body: ast::ModuleBody) -> Self {
        let mut component = Component { rewrite_nodes: vec![], name, diagnostics: vec![] };

        let mut matched_struct = false;
        for item in body.items(db).elements(db) {
            match &item {
                ast::Item::Struct(item_struct) => {
                    if matched_struct {
                        component.diagnostics.push(PluginDiagnostic {
                            message: "Only one struct per module is supported.".to_string(),
                            stable_ptr: item_struct.stable_ptr().untyped(),
                        });
                        continue;
                    }

                    component.handle_component_struct(db, item_struct.clone());
                    matched_struct = true;
                }
                ast::Item::FreeFunction(item_function) => {
                    component.handle_component_functions(db, item_function.clone());
                }
                _ => (),
            }
        }

        component
    }

    pub fn result(self, db: &dyn SyntaxGroup) -> PluginResult {
        let name = self.name;
        let mut builder = PatchBuilder::new(db);
        builder.add_modified(RewriteNode::interpolate_patched(
            &formatdoc!(
                "
                #[contract]
                mod {name} {{
                    $body$
                }}
                ",
            ),
            HashMap::from([(
                "body".to_string(),
                RewriteNode::Modified(ModifiedNode { children: self.rewrite_nodes }),
            )]),
        ));

        PluginResult {
            code: Some(PluginGeneratedFile {
                name,
                content: builder.code,
                aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(DojoAuxData {
                    patches: builder.patches,
                })),
            }),
            diagnostics: self.diagnostics,
            remove_original_item: true,
        }
    }

    fn handle_component_struct(&mut self, db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) {
        self.rewrite_nodes.push(RewriteNode::Copied(struct_ast.as_syntax_node()));

        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                    struct Storage {
                        world_address: felt,
                        state: LegacyMap::<felt, $type_name$>,
                    }
    
                    // Initialize $type_name$Component.
                    #[external]
                    fn initialize(world_addr: felt) {
                        let world = world_address::read();
                        assert(world == 0, '$type_name$Component: Already initialized.');
                        world_address::write(world_addr);
                    }
    
                    // Set the state of an entity.
                    #[external]
                    fn set(entity_id: felt, value: $type_name$) {
                        state::write(entity_id, value);
                    }
    
                    // Get the state of an entity.
                    #[view]
                    fn get(entity_id: felt) -> $type_name$ {
                        return state::read(entity_id);
                    }
                    ",
            HashMap::from([(
                "type_name".to_string(),
                RewriteNode::Trimmed(struct_ast.name(db).as_syntax_node()),
            )]),
        ));

        let mut serialize = vec![];
        let mut deserialize = vec![];
        let mut read = vec![];
        let mut write = vec![];
        struct_ast.members(db).elements(db).iter().enumerate().for_each(|(i, member)| {
            serialize.push(RewriteNode::interpolate_patched(
                "serde::Serde::<felt>::serialize(ref serialized, input.$key$);",
                HashMap::from([(
                    "key".to_string(),
                    RewriteNode::Trimmed(member.name(db).as_syntax_node()),
                )]),
            ));

            deserialize.push(RewriteNode::interpolate_patched(
                "$key$: serde::Serde::<felt>::deserialize(ref serialized)?,",
                HashMap::from([(
                    "key".to_string(),
                    RewriteNode::Trimmed(member.name(db).as_syntax_node()),
                )]),
            ));

            read.push(RewriteNode::interpolate_patched(
                "$key$: starknet::storage_read_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, \
                 $offset$_u8)
                )?,",
                HashMap::from([
                    ("key".to_string(), RewriteNode::Trimmed(member.name(db).as_syntax_node())),
                    ("offset".to_string(), RewriteNode::Text(i.to_string())),
                ]),
            ));

            let final_token =
                if i != struct_ast.members(db).elements(db).len() - 1 { "?;" } else { "" };
            write.push(RewriteNode::interpolate_patched(
                format!(
                    "
                    starknet::storage_write_syscall(
                        address_domain, starknet::storage_address_from_base_and_offset(base, \
                     $offset$_u8), value.$key$){final_token}"
                )
                .as_str(),
                HashMap::from([
                    ("key".to_string(), RewriteNode::Trimmed(member.name(db).as_syntax_node())),
                    ("offset".to_string(), RewriteNode::Text(i.to_string())),
                ]),
            ));
        });

        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                impl $type_name$Serde of serde::Serde::<$type_name$> {
                    fn serialize(ref serialized: Array::<felt>, input: $type_name$) {
                        $serialize$
                    }
                    fn deserialize(ref serialized: Array::<felt>) -> Option::<$type_name$> {
                        Option::Some(
                            $type_name$ {
                                $deserialize$
                            }
                        )
                    }
                }
            ",
            HashMap::from([
                (
                    "type_name".to_string(),
                    RewriteNode::Trimmed(struct_ast.name(db).as_syntax_node()),
                ),
                (
                    "serialize".to_string(),
                    RewriteNode::Modified(ModifiedNode { children: serialize }),
                ),
                (
                    "deserialize".to_string(),
                    RewriteNode::Modified(ModifiedNode { children: deserialize }),
                ),
            ]),
        ));

        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                impl StorageAccess$type_name$ of starknet::StorageAccess::<$type_name$> {
                    fn read(address_domain: felt, base: starknet::StorageBaseAddress) -> \
             starknet::SyscallResult::<$type_name$> {
                        Result::Ok(
                            $type_name$ {
                                $read$
                            }
                        )
                    }
                    fn write(
                        address_domain: felt, base: starknet::StorageBaseAddress, value: \
             $type_name$
                    ) -> starknet::SyscallResult::<()> {
                        $write$
                    }
                }
            ",
            HashMap::from([
                (
                    "type_name".to_string(),
                    RewriteNode::Trimmed(struct_ast.name(db).as_syntax_node()),
                ),
                ("read".to_string(), RewriteNode::Modified(ModifiedNode { children: read })),
                ("write".to_string(), RewriteNode::Modified(ModifiedNode { children: write })),
            ]),
        ));
    }

    fn handle_component_functions(&mut self, db: &dyn SyntaxGroup, func: ast::FunctionWithBody) {
        let declaration = func.declaration(db);

        let mut func_declaration = RewriteNode::from_ast(&declaration);
        func_declaration
            .modify_child(db, ast::FunctionDeclaration::INDEX_SIGNATURE)
            .modify_child(db, ast::FunctionSignature::INDEX_PARAMETERS)
            .modify(db)
            .children
            .splice(0..1, vec![RewriteNode::Text("entity_id: felt".to_string())]);

        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                            #[view]
                            $func_decl$ {
                                let self = state::read(entity_id);
                                $body$
                            }
                            ",
            HashMap::from([
                ("func_decl".to_string(), func_declaration),
                (
                    "body".to_string(),
                    RewriteNode::Trimmed(func.body(db).statements(db).as_syntax_node()),
                ),
            ]),
        ))
    }
}