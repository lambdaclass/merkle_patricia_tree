pub use self::{
    branch::{compute_branch_hash, BranchNode},
    extension::{compute_extension_hash, ExtensionNode},
    leaf::{compute_leaf_hash, LeafNode},
};

mod branch;
mod extension;
mod leaf;

#[cfg(test)]
#[macro_export]
macro_rules! pmt_tree {
    ( $value:ty ) => {
        $crate::PatriciaMerkleTree::<Vec<u8>, $value, sha3::Keccak256>::new()
    };
}

#[cfg(test)]
#[macro_export]
macro_rules! pmt_state {
    ( $value:ty ) => {
        (
            $crate::NodesStorage::<Vec<u8>, $value, sha3::Keccak256>::new(),
            $crate::ValuesStorage::<Vec<u8>, $value>::new(),
        )
    };
}

#[cfg(test)]
#[macro_export]
macro_rules! pmt_node {
    (
        @( $nodes:expr, $values:expr )
        branch { $( $choice:expr => $child_type:ident { $( $child_tokens:tt )* } ),+ $(,)? }
        $( offset $offset:expr )?
    ) => {
        $crate::nodes::BranchNode::<Vec<u8>, _, sha3::Keccak256>::new({
            #[allow(unused_variables)]
            let offset = true $( ^ $offset )?;
            let mut choices = [$crate::storage::NodeRef::default(); 16];
            $(
                let child_node = pmt_node! { @($nodes, $values)
                    $child_type { $( $child_tokens )* }
                    offset offset
                }.into();
                let child_node = $nodes.insert(child_node);
                choices[$choice as usize] = $crate::storage::NodeRef::new(child_node);
            )*
            choices
        })
    };
    (
        @( $nodes:expr, $values:expr )
        branch { $( $choice:expr => $child_type:ident { $( $child_tokens:tt )* } ),+ $(,)? }
        with_leaf { $path:expr => $value:expr }
        $( offset $offset:expr )?
    ) => {{
        let mut branch_node = $crate::nodes::BranchNode::<Vec<u8>, _, sha3::Keccak256>::new({
            #[allow(unused_variables)]
            let offset = true $( ^ $offset )?;
            let mut choices = [$crate::storage::NodeRef::default(); 16];
            $(
                choices[$choice as usize] = $crate::storage::NodeRef::new($nodes.insert(
                    pmt_node! { @($nodes, $values)
                        $child_type { $( $child_tokens )* }
                        offset offset
                    }.into()
                ));
            )*
            choices
        });
        branch_node.update_value_ref($crate::storage::ValueRef::new($values.insert(($path, $value))));
        branch_node
    }};

    (
        @( $nodes:expr, $values:expr )
        extension { $prefix:expr , $child_type:ident { $( $child_tokens:tt )* } }
        $( offset $offset:expr )?
    ) => {{
        #[allow(unused_variables)]
        let offset = false $( ^ $offset )?;
        let prefix = $crate::nibble::NibbleVec::from_nibbles(
            $prefix
                .into_iter()
                .map(|x: u8| $crate::nibble::Nibble::try_from(x).unwrap()),
            offset
        );

        let offset = offset  ^ (prefix.len() % 2 != 0);
        $crate::nodes::ExtensionNode::<Vec<u8>, _, sha3::Keccak256>::new(
            prefix,
            {
                let child_node = pmt_node! { @($nodes, $values)
                    $child_type { $( $child_tokens )* }
                    offset offset
                }.into();
                $crate::storage::NodeRef::new($nodes.insert(child_node))
            }
        )
    }};

    (
        @( $nodes:expr, $values:expr )
        leaf { $path:expr => $value:expr }
        $( offset $offset:expr )?
    ) => {
        $crate::nodes::LeafNode::<Vec<u8>, _, sha3::Keccak256>::new(
            $crate::storage::ValueRef::new($values.insert(($path, $value)))
        )
    };
}

#[cfg(test)]
#[macro_export]
macro_rules! pmt_path {
    ( $path:literal ) => {{
        assert!($path.len() % 2 == 1);
        $path
            .as_bytes()
            .chunks(2)
            .map(|bytes| u8::from_str_radix(std::str::from_utf8(bytes).unwrap(), 16).unwrap())
            .collect::<Vec<u8>>()
    }};
}
