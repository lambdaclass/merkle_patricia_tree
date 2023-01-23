//! Tests shamelessly copied from [here](https://github.com/ethereum/tests/blob/develop/TrieTests/trietest.json).
//!
//! Note: The commented lines within the tests are intentional. They correspond to the source's
//!   paths whose last inserts are null values, which are treated as remove operations. We don't
//!   implement removals, so we skipped inserting them to end up with the same root hash.

use hex_literal::hex;
use patricia_merkle_tree::PatriciaMerkleTree;
use sha3::Keccak256;

#[test]
fn empty_values() {
    // Note: The commented lines are intentional (see the header note).

    let mut tree = PatriciaMerkleTree::<&str, &str, Keccak256>::new();
    tree.insert("do", "verb");
    // tree.insert("ether", "wookiedoo");
    tree.insert("horse", "stallion");
    // tree.insert("shaman", "horse");
    tree.insert("doge", "coin");
    // tree.insert("ether", "");
    tree.insert("dog", "puppy");
    // tree.insert("shaman", "");

    assert_eq!(
        tree.compute_hash().as_slice(),
        hex!("5991bb8c6514148a29db676a14ac506cd2cd5775ace63c30a4fe457715e9ac84").as_slice()
    );
}

#[test]
fn branching_tests() {
    // Note: The commented lines are intentional (see the header note).

    let mut tree = PatriciaMerkleTree::<&[u8], &str, Keccak256>::new();
    // tree.insert(
    //     &hex!("04110d816c380812a427968ece99b1c963dfbce6"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("095e7baea6a6c7c4c2dfeb977efac326af552d87"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("0a517d755cebbf66312b30fff713666a9cb917e0"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("24dd378f51adc67a50e339e8031fe9bd4aafab36"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("293f982d000532a7861ab122bdc4bbfd26bf9030"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("2cf5732f017b0cf1b1f13a1478e10239716bf6b5"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("31c640b92c21a1f1465c91070b4b3b4d6854195f"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("37f998764813b136ddf5a754f34063fd03065e36"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("37fa399a749c121f8a15ce77e3d9f9bec8020d7a"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("4f36659fa632310b6ec438dea4085b522a2dd077"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("62c01474f089b07dae603491675dc5b5748f7049"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("729af7294be595a0efd7d891c9e51f89c07950c7"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("83e3e5a16d3b696a0314b30b2534804dd5e11197"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("8703df2417e0d7c59d063caa9583cb10a4d20532"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("8dffcd74e5b5923512916c6a64b502689cfa65e1"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("95a4d7cccb5204733874fa87285a176fe1e9e240"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("99b2fcba8120bedd048fe79f5262a6690ed38c39"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("a4202b8b8afd5354e3e40a219bdc17f6001bf2cf"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("a94f5374fce5edbc8e2a8697c15331677e6ebf0b"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("a9647f4a0a14042d91dc33c0328030a7157c93ae"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("aa6cffe5185732689c18f37a7f86170cb7304c2a"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("aae4a2e3c51c04606dcb3723456e58f3ed214f45"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("c37a43e940dfb5baf581a0b82b351d48305fc885"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("d2571607e241ecf590ed94b12d87c94babe36db6"),
    //     "something",
    // );
    // tree.insert(
    //     &hex!("f735071cbee190d76b704ce68384fc21e389fbe7"),
    //     "something",
    // );
    // tree.insert(&hex!("04110d816c380812a427968ece99b1c963dfbce6"), "");
    // tree.insert(&hex!("095e7baea6a6c7c4c2dfeb977efac326af552d87"), "");
    // tree.insert(&hex!("0a517d755cebbf66312b30fff713666a9cb917e0"), "");
    // tree.insert(&hex!("24dd378f51adc67a50e339e8031fe9bd4aafab36"), "");
    // tree.insert(&hex!("293f982d000532a7861ab122bdc4bbfd26bf9030"), "");
    // tree.insert(&hex!("2cf5732f017b0cf1b1f13a1478e10239716bf6b5"), "");
    // tree.insert(&hex!("31c640b92c21a1f1465c91070b4b3b4d6854195f"), "");
    // tree.insert(&hex!("37f998764813b136ddf5a754f34063fd03065e36"), "");
    // tree.insert(&hex!("37fa399a749c121f8a15ce77e3d9f9bec8020d7a"), "");
    // tree.insert(&hex!("4f36659fa632310b6ec438dea4085b522a2dd077"), "");
    // tree.insert(&hex!("62c01474f089b07dae603491675dc5b5748f7049"), "");
    // tree.insert(&hex!("729af7294be595a0efd7d891c9e51f89c07950c7"), "");
    // tree.insert(&hex!("83e3e5a16d3b696a0314b30b2534804dd5e11197"), "");
    // tree.insert(&hex!("8703df2417e0d7c59d063caa9583cb10a4d20532"), "");
    // tree.insert(&hex!("8dffcd74e5b5923512916c6a64b502689cfa65e1"), "");
    // tree.insert(&hex!("95a4d7cccb5204733874fa87285a176fe1e9e240"), "");
    // tree.insert(&hex!("99b2fcba8120bedd048fe79f5262a6690ed38c39"), "");
    // tree.insert(&hex!("a4202b8b8afd5354e3e40a219bdc17f6001bf2cf"), "");
    // tree.insert(&hex!("a94f5374fce5edbc8e2a8697c15331677e6ebf0b"), "");
    // tree.insert(&hex!("a9647f4a0a14042d91dc33c0328030a7157c93ae"), "");
    // tree.insert(&hex!("aa6cffe5185732689c18f37a7f86170cb7304c2a"), "");
    // tree.insert(&hex!("aae4a2e3c51c04606dcb3723456e58f3ed214f45"), "");
    // tree.insert(&hex!("c37a43e940dfb5baf581a0b82b351d48305fc885"), "");
    // tree.insert(&hex!("d2571607e241ecf590ed94b12d87c94babe36db6"), "");
    // tree.insert(&hex!("f735071cbee190d76b704ce68384fc21e389fbe7"), "");

    assert_eq!(
        tree.compute_hash().as_slice(),
        hex!("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421").as_slice(),
    );
}

#[test]
fn jeff() {
    // Note: The commented lines are intentional (see the header note).

    let mut tree = PatriciaMerkleTree::<&[u8], &[u8], Keccak256>::new();
    tree.insert(
        &hex!("0000000000000000000000000000000000000000000000000000000000000045"),
        &hex!("22b224a1420a802ab51d326e29fa98e34c4f24ea"),
    );
    tree.insert(
        &hex!("0000000000000000000000000000000000000000000000000000000000000046"),
        &hex!("67706c2076330000000000000000000000000000000000000000000000000000"),
    );
    // tree.insert(
    //     &hex!("0000000000000000000000000000000000000000000000000000001234567890"),
    //     &hex!("697c7b8c961b56f675d570498424ac8de1a918f6"),
    // );
    tree.insert(
        &hex!("000000000000000000000000697c7b8c961b56f675d570498424ac8de1a918f6"),
        &hex!("1234567890"),
    );
    tree.insert(
        &hex!("0000000000000000000000007ef9e639e2733cb34e4dfc576d4b23f72db776b2"),
        &hex!("4655474156000000000000000000000000000000000000000000000000000000"),
    );
    tree.insert(
        &hex!("000000000000000000000000ec4f34c97e43fbb2816cfd95e388353c7181dab1"),
        &hex!("4e616d6552656700000000000000000000000000000000000000000000000000"),
    );
    tree.insert(
        &hex!("4655474156000000000000000000000000000000000000000000000000000000"),
        &hex!("7ef9e639e2733cb34e4dfc576d4b23f72db776b2"),
    );
    tree.insert(
        &hex!("4e616d6552656700000000000000000000000000000000000000000000000000"),
        &hex!("ec4f34c97e43fbb2816cfd95e388353c7181dab1"),
    );
    // tree.insert(
    //     &hex!("0000000000000000000000000000000000000000000000000000001234567890"),
    //     &"",
    // );
    tree.insert(
        &hex!("000000000000000000000000697c7b8c961b56f675d570498424ac8de1a918f6"),
        &hex!("6f6f6f6820736f2067726561742c207265616c6c6c793f000000000000000000"),
    );
    tree.insert(
        &hex!("6f6f6f6820736f2067726561742c207265616c6c6c793f000000000000000000"),
        &hex!("697c7b8c961b56f675d570498424ac8de1a918f6"),
    );

    assert_eq!(
        tree.compute_hash().as_slice(),
        hex!("9f6221ebb8efe7cff60a716ecb886e67dd042014be444669f0159d8e68b42100").as_slice(),
    );
}

#[test]
fn insert_middle_leaf() {
    let mut tree = PatriciaMerkleTree::<&str, &str, Keccak256>::new();
    tree.insert("key1aa", "0123456789012345678901234567890123456789xxx");
    tree.insert("key1", "0123456789012345678901234567890123456789Very_Long");
    tree.insert("key2bb", "aval3");
    tree.insert("key2", "short");
    tree.insert("key3cc", "aval3");
    tree.insert("key3", "1234567890123456789012345678901");

    assert_eq!(
        tree.compute_hash().as_slice(),
        hex!("cb65032e2f76c48b82b5c24b3db8f670ce73982869d38cd39a624f23d62a9e89").as_slice(),
    );
}

#[test]
fn branch_value_update() {
    let mut tree = PatriciaMerkleTree::<&str, &str, Keccak256>::new();
    tree.insert("abc", "123");
    tree.insert("abcd", "abcd");
    tree.insert("abc", "abc");

    assert_eq!(
        tree.compute_hash().as_slice(),
        hex!("7a320748f780ad9ad5b0837302075ce0eeba6c26e3d8562c67ccc0f1b273298a").as_slice(),
    );
}
