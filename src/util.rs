#![warn(warnings)]

use crate::nibble::Nibble;
use digest::{Digest, Output};

pub fn compute_hash_from_sorted_iter<'a, P, V, H>(
    iter: impl IntoIterator<Item = (&'a P, &'a V)>,
) -> Output<H>
where
    P: 'a + AsRef<[u8]>,
    V: 'a + AsRef<[u8]>,
    H: Digest,
{
    let iter = iter.into_iter();

    let mut nodes_stack: Vec<StackItem<H>> = Vec::new();
    let mut actual_path: (&[u8], Option<Nibble>) = (b"", None);
    let mut nodes_count: usize = 0;

    for (path, value) in iter {
        let path = path.as_ref();
        let value = value.as_ref();

        if nodes_stack.is_empty() {
            nodes_stack.push(StackItem::RawNode(path, value));
            nodes_count += 1;

            actual_path = (
                &path[..path.len() - 1],
                path.last().map(|x| Nibble::try_from(x >> 4).unwrap()),
            );
        } else {
            let mut offset = path
                .iter()
                .zip(actual_path.0)
                .take_while(|(a, b)| a == b)
                .count();

            offset *= 2;

            let actual_path_len = 2 * actual_path.0.len() + actual_path.1.is_some() as usize;
            if offset + 1 == actual_path_len {
                if path.get(offset >> 1).map(|x| x >> 4) == actual_path.1.map(u8::from) {
                    offset += 1;
                }
            } else if path.get(offset >> 1).map(|x| x >> 4)
                == actual_path.0.get(offset >> 1).map(|x| x >> 4)
            {
                offset += 1;
            }

            assert!(
                path.len() <= (2 * actual_path.0.len() + 1 + actual_path.1.is_some() as usize) >> 1,
                "iter is not sorted",
            );
            // TODO Check sorted values when pushing choices.
            println!("offset = {offset}, actual_path_len = {actual_path_len}");

            if offset == actual_path_len {
                if path.get(offset >> 1).is_some() {
                    nodes_stack.push(StackItem::RawNode(path, value));
                    nodes_count += 1;
                } else {
                    todo!("branch with value")
                }
            } else {
                for node in &nodes_stack[nodes_stack.len() - nodes_count..] {
                    // TODO: Encode node (replacing the value in the stack).
                }
                // TODO: Encode branch (with values from stack).
                nodes_stack.truncate(nodes_stack.len() - nodes_count);
                nodes_count = 0;

                if offset + 1 < actual_path_len {
                    // TODO: Encode extension.
                    // TODO: Push encoded extension.
                    println!("extension (prefix len is {})", actual_path_len - offset);
                }
            }
        }

        println!("path = {path:02x?}, actual_path = {actual_path:02x?}");
    }

    if nodes_count > 0 {
        // TODO: Encode branch.
        // TODO: Maybe encode prefix.
    }

    // TODO: Keep encoding until the root (somehow...).
    // TODO: Maybe hash the root.

    todo!()
}

enum StackItem<'a, H>
where
    H: Digest,
{
    RawNode(&'a [u8], &'a [u8]),
    NodeRef(Output<H>, usize),
}

#[cfg(test)]
mod test {
    use super::*;
    use hex_literal::hex;
    use sha3::Keccak256;

    #[test]
    fn test() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&hex!("0123"), &hex!("0123")),
            (&hex!("0124"), &hex!("0124")),
            (&hex!("0246"), &hex!("0246")),
            // (&hex!("1234"), &hex!("1234")),
        ];

        compute_hash_from_sorted_iter::<&[u8], &[u8], Keccak256>(
            DATA //
                .iter()
                .map(|(a, b)| (a, b)),
        );
    }
}
