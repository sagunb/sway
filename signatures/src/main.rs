use anyhow::Result;
use secp256k1_test::{Message as SecpMessage, Secp256k1};
use sha256::digest_bytes;

// Use the `tokio::main` macro for using async on the main function
#[tokio::main]
async fn main() -> Result<()> {
    let secp256k1 = Secp256k1::new();
    let message_arr = [42u8; 32];
    let secret_key = secp256k1_test::key::ONE_KEY; // the number 1 as a secret key
    let public_key = secp256k1_test::key::PublicKey::from_secret_key(&secp256k1, &secret_key);
    let message = SecpMessage::from_slice(&message_arr).unwrap();
    let signature = secp256k1.sign_recoverable(&message, &secret_key);

    let pubkey_a = public_key.serialize_uncompressed();
    let address = digest_bytes(&pubkey_a);
    assert_eq!(pubkey_a.len(), 65);

    let (_rec_id, signature_a) = signature.serialize_compact();
    let hex_sig = hex::encode(signature_a);
    assert_eq!(signature_a.len(), 64);

    println!("privkey: {}", secret_key);
    println!("pubkey: {}", public_key);
    println!("message: {:?}", message);
    println!("Address: {:?}", address);
    println!("64-byte Hex Signature: {:?}", hex_sig);
    assert_eq!(hex_sig.len(), 128);

    Ok(())
}
