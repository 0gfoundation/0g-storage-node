use crate::sha3::Sha3Algorithm;
use crate::{Proof, RangeProof};
use anyhow::{bail, Result};
use ethereum_types::H256;
use once_cell::sync::Lazy;
use ssz::{Decode, Encode};
use std::fmt::Debug;
use std::hash::Hash;
use tracing::trace;

/// A wrapper around Option<H256> that properly handles null hashes
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct OptionalHash(pub Option<H256>);

impl OptionalHash {
    pub fn some(hash: H256) -> Self {
        OptionalHash(Some(hash))
    }

    pub fn none() -> Self {
        OptionalHash(None)
    }

    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    pub fn unwrap(&self) -> H256 {
        self.0.unwrap()
    }

    pub fn unwrap_or(&self, default: H256) -> H256 {
        self.0.unwrap_or(default)
    }

    pub fn as_ref(&self) -> Option<&H256> {
        self.0.as_ref()
    }

    pub fn to_h256_or_zero(&self) -> H256 {
        self.0.unwrap_or_else(H256::zero)
    }

    /// Convert a Proof<OptionalHash> to Proof<H256>
    pub fn convert_proof_to_h256(proof: Proof<OptionalHash>) -> Result<Proof<H256>, anyhow::Error> {
        let lemma: Vec<H256> = proof
            .lemma()
            .iter()
            .map(|oh| oh.to_h256_or_zero())
            .collect();
        let path = proof.path().to_vec();
        Proof::new(lemma, path)
    }

    /// Convert a Proof<H256> to Proof<OptionalHash>
    pub fn convert_proof_from_h256(
        proof: Proof<H256>,
    ) -> Result<Proof<OptionalHash>, anyhow::Error> {
        let lemma: Vec<OptionalHash> = proof
            .lemma()
            .iter()
            .map(|h| OptionalHash::some(*h))
            .collect();
        let path = proof.path().to_vec();
        Proof::new(lemma, path)
    }

    /// Convert a RangeProof<H256> to RangeProof<OptionalHash>
    pub fn convert_range_proof_from_h256(
        range_proof: RangeProof<H256>,
    ) -> Result<RangeProof<OptionalHash>, anyhow::Error> {
        Ok(RangeProof {
            left_proof: Self::convert_proof_from_h256(range_proof.left_proof)?,
            right_proof: Self::convert_proof_from_h256(range_proof.right_proof)?,
        })
    }
}

// Add From conversions for easier usage
impl From<H256> for OptionalHash {
    fn from(hash: H256) -> Self {
        OptionalHash::some(hash)
    }
}

impl From<Option<H256>> for OptionalHash {
    fn from(opt: Option<H256>) -> Self {
        OptionalHash(opt)
    }
}

impl From<OptionalHash> for Option<H256> {
    fn from(opt_hash: OptionalHash) -> Self {
        opt_hash.0
    }
}

impl AsRef<[u8]> for OptionalHash {
    fn as_ref(&self) -> &[u8] {
        match &self.0 {
            Some(hash) => hash.as_ref(),
            None => &[0u8; 32], // Return zeros for null hash
        }
    }
}

impl AsMut<[u8]> for OptionalHash {
    fn as_mut(&mut self) -> &mut [u8] {
        if self.0.is_none() {
            self.0 = Some(H256::zero());
        }
        self.0.as_mut().unwrap().as_mut()
    }
}

impl Encode for OptionalHash {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_fixed_len() -> usize {
        33 // 1 byte for Some/None + 32 bytes for hash
    }

    fn ssz_bytes_len(&self) -> usize {
        33
    }

    fn ssz_append(&self, buf: &mut Vec<u8>) {
        match &self.0 {
            Some(hash) => {
                buf.push(1); // Some discriminant
                hash.ssz_append(buf);
            }
            None => {
                buf.push(0); // None discriminant
                buf.extend_from_slice(&[0u8; 32]);
            }
        }
    }
}

impl Decode for OptionalHash {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_fixed_len() -> usize {
        33
    }

    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, ssz::DecodeError> {
        if bytes.len() != 33 {
            return Err(ssz::DecodeError::InvalidByteLength {
                len: bytes.len(),
                expected: 33,
            });
        }

        match bytes[0] {
            0 => Ok(OptionalHash::none()),
            1 => {
                let hash = H256::from_ssz_bytes(&bytes[1..])?;
                Ok(OptionalHash::some(hash))
            }
            _ => Err(ssz::DecodeError::BytesInvalid(
                "Invalid discriminant for OptionalHash".to_string(),
            )),
        }
    }
}

unsafe impl Send for OptionalHash {}
unsafe impl Sync for OptionalHash {}

pub trait HashElement:
    Clone + Debug + Eq + Hash + AsRef<[u8]> + AsMut<[u8]> + Decode + Encode + Send + Sync
{
    fn end_pad(height: usize) -> Self;
    fn null() -> Self;
    fn is_null(&self) -> bool {
        self == &Self::null()
    }
}

impl HashElement for OptionalHash {
    fn end_pad(height: usize) -> Self {
        OptionalHash::some(ZERO_HASHES[height])
    }

    fn null() -> Self {
        OptionalHash::none()
    }

    fn is_null(&self) -> bool {
        self.is_none()
    }
}

// Keep the H256 implementation for backward compatibility
impl HashElement for H256 {
    fn end_pad(height: usize) -> Self {
        ZERO_HASHES[height]
    }

    fn null() -> Self {
        H256::zero() // Use all zeros instead of 0x0101... to avoid collision
    }
}

pub static ZERO_HASHES: Lazy<[H256; 64]> = Lazy::new(|| {
    let leaf_zero_hash: H256 = Sha3Algorithm::leaf_raw(&[0u8; 256]);
    let mut list = [H256::zero(); 64];
    list[0] = leaf_zero_hash;
    for i in 1..list.len() {
        list[i] = Sha3Algorithm::parent_raw(&list[i - 1], &list[i - 1]);
    }
    list
});

pub trait Algorithm<E: HashElement> {
    fn parent(left: &E, right: &E) -> E;
    fn parent_single(r: &E, height: usize) -> E {
        let right = E::end_pad(height);
        Self::parent(r, &right)
    }
    fn leaf(data: &[u8]) -> E;
}

pub trait MerkleTreeRead {
    type E: HashElement;
    fn node(&self, layer: usize, index: usize) -> Self::E;
    fn height(&self) -> usize;
    fn layer_len(&self, layer_height: usize) -> usize;
    fn padding_node(&self, height: usize) -> Self::E;

    fn leaves(&self) -> usize {
        self.layer_len(0)
    }

    fn root(&self) -> Self::E {
        self.node(self.height() - 1, 0)
    }

    fn gen_proof(&self, leaf_index: usize) -> Result<Proof<Self::E>> {
        if leaf_index >= self.leaves() {
            bail!(
                "leaf index out of bound: leaf_index={} total_leaves={}",
                leaf_index,
                self.leaves()
            );
        }
        if self.node(0, leaf_index).is_null() {
            bail!("Not ready to generate proof for leaf_index={}", leaf_index);
        }
        if self.height() == 1 {
            return Proof::new(vec![self.root(), self.root().clone()], vec![]);
        }
        let mut lemma: Vec<Self::E> = Vec::with_capacity(self.height()); // path + root
        let mut path: Vec<bool> = Vec::with_capacity(self.height() - 2); // path - 1
        let mut index_in_layer = leaf_index;
        lemma.push(self.node(0, leaf_index));
        for height in 0..(self.height() - 1) {
            trace!(
                "gen_proof: height={} index={} hash={:?}",
                height,
                index_in_layer,
                self.node(height, index_in_layer)
            );
            if index_in_layer % 2 == 0 {
                path.push(true);
                if index_in_layer + 1 == self.layer_len(height) {
                    // TODO: This can be skipped if the tree size is available in validation.
                    lemma.push(self.padding_node(height));
                } else {
                    lemma.push(self.node(height, index_in_layer + 1));
                }
            } else {
                path.push(false);
                lemma.push(self.node(height, index_in_layer - 1));
            }
            index_in_layer >>= 1;
        }
        lemma.push(self.root());
        if lemma.iter().any(|e| e.is_null()) {
            bail!(
                "Not enough data to generate proof, lemma={:?} path={:?}",
                lemma,
                path
            );
        }
        Proof::new(lemma, path)
    }

    fn gen_range_proof(&self, start_index: usize, end_index: usize) -> Result<RangeProof<Self::E>> {
        if end_index <= start_index {
            bail!(
                "invalid proof range: start={} end={}",
                start_index,
                end_index
            );
        }
        // TODO(zz): Optimize range proof.
        let left_proof = self.gen_proof(start_index)?;
        let right_proof = self.gen_proof(end_index - 1)?;
        Ok(RangeProof {
            left_proof,
            right_proof,
        })
    }
}

pub trait MerkleTreeWrite {
    type E: HashElement;
    fn push_node(&mut self, layer: usize, node: Self::E);
    fn append_nodes(&mut self, layer: usize, nodes: &[Self::E]);
    fn update_node(&mut self, layer: usize, pos: usize, node: Self::E);
}

/// This includes the data to reconstruct an `AppendMerkleTree` root where some nodes
/// are `null`. Other intermediate nodes will be computed based on these known nodes.
pub struct MerkleTreeInitialData<E: HashElement> {
    /// A list of `(subtree_depth, root)`.
    /// The subtrees are continuous so we can compute the tree root with these subtree roots.
    pub subtree_list: Vec<(usize, E)>,
    /// A list of `(index, leaf_hash)`.
    /// These leaves are in some large subtrees of `subtree_list`. 1-node subtrees are also leaves,
    /// but they will not be duplicated in `known_leaves`.
    pub known_leaves: Vec<(usize, E)>,

    /// A list of `(layer_index, position, hash)`.
    /// These are the nodes known from proofs.
    /// They should only be inserted after constructing the tree.
    pub extra_mpt_nodes: Vec<(usize, usize, E)>,
}

impl<E: HashElement> MerkleTreeInitialData<E> {
    pub fn leaves(&self) -> usize {
        self.subtree_list.iter().fold(0, |acc, (subtree_depth, _)| {
            acc + (1 << (subtree_depth - 1))
        })
    }
}
