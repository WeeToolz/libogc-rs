// Fixed values from the console configuration ABI, independent expected bytes.
const CONF_ENOENT: i32 = -0x6002;
const CONF_ETOOBIG: i32 = -0x6003;
#[path = "../../crates/core/ogc-conf/src/text_settings.rs"]
mod text;
#[test]
fn model_field_preserves_settings_and_rejects_truncation() {
    let mut output = [0; 32];
    let input = b"AREA=USA\r\nMODEL=RVL-101(USA)\r\n";
    assert_eq!(text::get_txt_from_data("MODEL", input, &mut output), Ok(12));
    assert_eq!(&output[..13], b"RVL-101(USA)\0");
    assert_eq!(text::get_txt_from_data("MODEL", input, &mut [0; 12]), Err(-0x6003));
    assert_eq!(text::get_txt_from_data("MODEL", b"AREA=USA\r\n", &mut output), Err(-0x6002));
}
