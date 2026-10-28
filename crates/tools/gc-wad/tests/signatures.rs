use gc_wad::{SignatureStatus, Ticket, signature_status};
#[test]
fn verifies_openssl_rsa_signature_and_rejects_changed_body_and_signature() {
    let ticket = include_bytes!("fixtures/rsa-ticket.bin");
    let certificate = include_bytes!("fixtures/rsa-cert.bin");
    Ticket::parse(ticket).unwrap();
    assert_eq!(
        signature_status(ticket, certificate).unwrap(),
        SignatureStatus::ValidRsa
    );
    let mut altered = *ticket;
    altered[0x200] ^= 1;
    assert_eq!(
        signature_status(&altered, certificate).unwrap(),
        SignatureStatus::Invalid
    );
    let mut altered = *ticket;
    altered[4] ^= 1;
    assert_eq!(
        signature_status(&altered, certificate).unwrap(),
        SignatureStatus::Invalid
    );
    assert!(signature_status(ticket, &certificate[..certificate.len() - 1]).is_err());
}
