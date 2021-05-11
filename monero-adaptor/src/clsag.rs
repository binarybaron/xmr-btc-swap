use crate::ring::Ring;
use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::edwards::{CompressedEdwardsY, EdwardsPoint};
use curve25519_dalek::scalar::Scalar;
use hash_edwards_to_edwards::hash_point_to_point;
use tiny_keccak::{Hasher, Keccak};

pub const RING_SIZE: usize = 11;

const INV_EIGHT: Scalar = Scalar::from_bits([
    121, 47, 220, 226, 41, 229, 6, 97, 208, 218, 28, 125, 179, 157, 211, 7, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 6,
]);

pub fn sign(
    msg: &[u8],
    signing_key: Scalar,
    H_p_pk: EdwardsPoint,
    alpha: Scalar,
    ring: Ring,
    commitment_ring: Ring,
    fake_responses: [Scalar; RING_SIZE - 1],
    z: Scalar,
    pseudo_output_commitment: EdwardsPoint,
    L_0: EdwardsPoint,
    R_0: EdwardsPoint,
    I: EdwardsPoint,
) -> Signature {
    let D = z * H_p_pk;
    let D_inv_8 = D * INV_EIGHT;

    let mus = AggregationHashes::new(
        &ring,
        &commitment_ring,
        I.compress(),
        pseudo_output_commitment.compress(),
        H_p_pk.compress(),
    );

    let prefix = clsag_round_hash_prefix(
        ring.as_ref(),
        commitment_ring.as_ref(),
        pseudo_output_commitment,
        msg,
    );
    let h_0 = hash_to_scalar(&[
        &prefix,
        L_0.compress().as_bytes(),
        R_0.compress().as_bytes(),
    ]);

    let h_last = fake_responses
        .iter()
        .enumerate()
        .fold(h_0, |h_prev, (i, s_i)| {
            let pk_i = ring[i + 1];
            let adjusted_commitment_i = commitment_ring[i] - pseudo_output_commitment;

            let L_i = compute_L(h_prev, &mus, *s_i, pk_i, adjusted_commitment_i);
            let R_i = compute_R(h_prev, &mus, pk_i, *s_i, I, D_inv_8);

            hash_to_scalar(&[
                &prefix,
                L_i.compress().as_bytes().as_ref(),
                R_i.compress().as_bytes().as_ref(),
            ])
        });

    let s_last = alpha - h_last * ((mus.mu_P * signing_key) + (mus.mu_C * z));

    Signature {
        responses: [
            fake_responses[0],
            fake_responses[1],
            fake_responses[2],
            fake_responses[3],
            fake_responses[4],
            fake_responses[5],
            fake_responses[6],
            fake_responses[7],
            fake_responses[8],
            fake_responses[9],
            s_last,
        ],
        h_0,
        I,
        D,
    }
}

#[must_use]
pub fn verify(sig: &Signature, ring: [EdwardsPoint; RING_SIZE], msg: &[u8]) -> bool {
    let ring_concat = ring
        .iter()
        .flat_map(|pk| pk.compress().as_bytes().to_vec())
        .collect::<Vec<u8>>();

    let mut h = sig.h_0;

    let mus = todo!();
    let adjusted_commitment_i = todo!();

    for (i, s_i) in sig.responses.iter().enumerate() {
        let pk_i = ring[(i + 1) % RING_SIZE];
        let prefix = clsag_round_hash_prefix(&ring_concat, todo!(), todo!(), msg);
        let L_i = compute_L(h, mus, *s_i, pk_i, adjusted_commitment_i);
        let R_i = compute_R(h, mus, pk_i, *s_i, sig.I, sig.D);

        h = hash_to_scalar(&[
            &prefix,
            L_i.compress().as_bytes().as_ref(),
            R_i.compress().as_bytes().as_ref(),
        ])
    }

    h == sig.h_0
}

pub struct Signature {
    pub responses: [Scalar; RING_SIZE],
    pub h_0: Scalar,
    /// Key image of the real key in the ring.
    pub I: EdwardsPoint,
    pub D: EdwardsPoint,
}

/// Compute the prefix for the hash common to every iteration of the ring
/// signature algorithm.
///
/// "CLSAG_round" || ring || ring of commitments || pseudooutput commitment ||
/// msg || alpha * G
fn clsag_round_hash_prefix(
    ring: &[u8],
    commitment_ring: &[u8],
    pseudo_output_commitment: EdwardsPoint,
    msg: &[u8],
) -> Vec<u8> {
    let domain_prefix = b"CLSAG_round";
    let pseudo_output_commitment = pseudo_output_commitment.compress();
    let pseudo_output_commitment = pseudo_output_commitment.as_bytes();

    let mut prefix = Vec::with_capacity(
        domain_prefix.len()
            + ring.len()
            + commitment_ring.len()
            + pseudo_output_commitment.len()
            + msg.len(),
    );

    prefix.extend(domain_prefix);
    prefix.extend(ring);
    prefix.extend(commitment_ring);
    prefix.extend(pseudo_output_commitment);
    prefix.extend(msg);

    prefix
}

// L_i = s_i * G + c_p * pk_i + c_c * (commitment_i - pseudoutcommitment)
fn compute_L(
    h_prev: Scalar,
    mus: &AggregationHashes,
    s_i: Scalar,
    pk_i: EdwardsPoint,
    adjusted_commitment_i: EdwardsPoint,
) -> EdwardsPoint {
    let c_p = h_prev * mus.mu_P;
    let c_c = h_prev * mus.mu_C;

    (s_i * ED25519_BASEPOINT_POINT) + (c_p * pk_i) + c_c * adjusted_commitment_i
}

// R_i = s_i * H_p_pk_i + c_p * I + c_c * (z * hash_to_point(signing pk))
fn compute_R(
    h_prev: Scalar,
    mus: &AggregationHashes,
    pk_i: EdwardsPoint,
    s_i: Scalar,
    I: EdwardsPoint,
    D: EdwardsPoint,
) -> EdwardsPoint {
    let c_p = h_prev * mus.mu_P;
    let c_c = h_prev * mus.mu_C;

    let H_p_pk_i = hash_point_to_point(pk_i);

    (s_i * H_p_pk_i) + (c_p * I) + c_c * D
}

struct AggregationHashes {
    mu_P: Scalar,
    mu_C: Scalar,
}

impl AggregationHashes {
    fn new(
        ring: &Ring,
        commitment_ring: &Ring,
        I: CompressedEdwardsY,
        pseudo_output_commitment: CompressedEdwardsY,
        D: CompressedEdwardsY,
    ) -> Self {
        let ring = ring.as_ref();
        let commitment_ring = commitment_ring.as_ref();
        let I = I.as_bytes().as_ref();
        let D = D.as_bytes().as_ref();
        let pseudo_output_commitment = pseudo_output_commitment.as_bytes().as_ref();

        let mu_P = hash_to_scalar(&[
            b"CLSAG_agg_0",
            ring,
            commitment_ring,
            I,
            D,
            pseudo_output_commitment,
        ]);
        let mu_C = hash_to_scalar(&[
            b"CLSAG_agg_1",
            ring,
            commitment_ring,
            I,
            D,
            pseudo_output_commitment,
        ]);

        Self { mu_P, mu_C }
    }
}

impl From<Signature> for monero::util::ringct::Clsag {
    fn from(from: Signature) -> Self {
        Self {
            s: from
                .responses
                .iter()
                .map(|s| monero::util::ringct::Key { key: s.to_bytes() })
                .collect(),
            c1: monero::util::ringct::Key {
                key: from.h_0.to_bytes(),
            },
            D: monero::util::ringct::Key {
                key: from.D.compress().to_bytes(),
            },
        }
    }
}

fn hash_to_scalar(elements: &[&[u8]]) -> Scalar {
    let mut hasher = Keccak::v256();

    for element in elements {
        hasher.update(element);
    }

    let mut hash = [0u8; 32];
    hasher.finalize(&mut hash);

    Scalar::from_bytes_mod_order(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn const_is_inv_eight() {
        let inv_eight = Scalar::from(8u8).invert();

        assert_eq!(inv_eight, INV_EIGHT);
    }

    #[test]
    fn sign_and_verify() {
        let msg_to_sign = b"hello world, monero is amazing!!";

        let s_prime_a = Scalar::random(&mut OsRng);
        let s_b = Scalar::random(&mut OsRng);

        let pk = (s_prime_a + s_b) * ED25519_BASEPOINT_POINT;

        let (r_a, R_a, R_prime_a) = {
            let r_a = Scalar::random(&mut OsRng);
            let R_a = r_a * ED25519_BASEPOINT_POINT;

            let pk_hashed_to_point = hash_point_to_point(pk);

            let R_prime_a = r_a * pk_hashed_to_point;

            (r_a, R_a, R_prime_a)
        };

        let mut ring = [EdwardsPoint::default(); RING_SIZE];
        ring[0] = pk;

        ring[1..].fill_with(|| {
            let x = Scalar::random(&mut OsRng);
            x * ED25519_BASEPOINT_POINT
        });

        let mut commitment_ring = [EdwardsPoint::default(); RING_SIZE];

        let real_commitment_blinding = Scalar::random(&mut OsRng);
        commitment_ring[0] = real_commitment_blinding * ED25519_BASEPOINT_POINT; // + 0 * H
        commitment_ring[1..].fill_with(|| {
            let x = Scalar::random(&mut OsRng);
            x * ED25519_BASEPOINT_POINT
        });

        // TODO: document
        let pseudo_output_commitment = commitment_ring[0];

        let signature = sign(msg_to_sign, todo!(), todo!(), todo!(), todo!(), todo!(), todo!(), todo!(), todo!(), todo!(), todo!(), todo!());

        assert!(verify(&signature, todo!(), todo!()))
    }
}