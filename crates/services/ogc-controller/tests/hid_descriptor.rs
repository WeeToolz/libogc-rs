use ogc_controller::hid::{HidError, HidReportMap};

#[test]
fn decodes_unsigned_buttons_and_signed_axes() {
    // Button 1..8, then signed 8-bit X and Y. Input bytes are fixed protocol data.
    let descriptor = [
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x08, 0x15, 0x00, 0x25,
        0x01, 0x95, 0x08, 0x75, 0x01, 0x81, 0x02, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x15, 0x81,
        0x25, 0x7f, 0x75, 0x08, 0x95, 0x02, 0x81, 0x02, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    let input = map.decode(&[0x81, 0x81, 0x7f]).unwrap();
    assert_eq!(input.button_usages, 0x81);
    assert_eq!(input.present_button_usages, 0xff);
    assert_eq!(input.axes, [-32768, 32767, 0, 0, 0, 0]);
    assert_eq!(input.present_axes, 0b11);
    assert_eq!(input.control_count, 10);
    let released = map.decode(&[0, 0, 0]).unwrap();
    assert_eq!(released.button_usages, 0);
    assert_eq!(released.present_button_usages, 0xff);
}

#[test]
fn decodes_fields_packed_at_nibble_offsets() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x04, 0xa1, 0x01, 0x09, 0x30, 0x09, 0x31, 0x15, 0xf8, 0x25, 0x07, 0x75,
        0x04, 0x95, 0x02, 0x82, 0x02, 0x00, 0xc0,
    ];
    let input = HidReportMap::parse(&descriptor)
        .unwrap()
        .decode(&[0x78])
        .unwrap();
    assert_eq!(input.axes, [-32768, 32767, 0, 0, 0, 0]);
}

#[test]
fn keeps_report_ids_separate_and_requires_the_id_prefix() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x01, 0x05, 0x09, 0x09, 0x01, 0x15, 0x00, 0x25,
        0x01, 0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x85, 0x02, 0x05, 0x01, 0x09, 0x30, 0x15, 0x00,
        0x25, 0xff, 0x75, 0x08, 0x95, 0x01, 0x81, 0x02, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    let button = map.decode(&[0x01, 0x01]).unwrap();
    assert_eq!(button.button_usages, 1);
    assert_eq!(button.present_axes, 0);
    let axis = map.decode(&[0x02, 0xff]).unwrap();
    assert_eq!(axis.report_id, 2);
    assert_eq!(axis.present_axes, 1);
    assert_eq!(axis.axes[0], 32767);
    assert_eq!(map.required_input_bytes(1).unwrap(), 2);
    assert_eq!(map.required_input_bytes(2).unwrap(), 2);
    assert_eq!(map.maximum_input_bytes(), 2);
    assert!(map.application_capabilities(1).unwrap().gamepad);
    assert!(map.application_capabilities(2).unwrap().gamepad);
    assert_eq!(
        map.decode(&[0xff, 0]).unwrap_err(),
        HidError::UnknownReportId
    );
    assert_eq!(map.decode(&[0x01]).unwrap_err(), HidError::ReportLength);
}

#[test]
fn hat_null_value_is_absent_and_usage_controls_are_preserved() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x09, 0x39, 0x15, 0x01, 0x25, 0x08, 0x35, 0x00, 0x46,
        0x3b, 0x01, 0x75, 0x04, 0x95, 0x01, 0x81, 0x42, 0x05, 0xff, 0x09, 0x22, 0x15, 0x00, 0x25,
        0x0f, 0x75, 0x04, 0x95, 0x01, 0x81, 0x02, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    let null = map.decode(&[0x00]).unwrap();
    assert_eq!(null.hat, None);
    assert!(null.hat_present);
    assert_eq!(null.controls[0].usage, 0x39);
    assert_eq!(null.controls[0].value, 0);
    let control = map.decode(&[0x71]).unwrap();
    assert_eq!(control.hat, Some(0));
    assert_eq!(control.controls[1].usage_page, 0xff);
    assert_eq!(control.controls[1].usage, 0x22);
    assert_eq!(control.controls[1].value, 7);
}

#[test]
fn decodes_keyboard_usages_and_rejects_rollover() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x19, 0xe0, 0x29, 0xe7, 0x15, 0x00, 0x25,
        0x01, 0x75, 0x01, 0x95, 0x08, 0x81, 0x02, 0x75, 0x08, 0x95, 0x01, 0x81, 0x01, 0x19, 0x00,
        0x29, 0x65, 0x15, 0x00, 0x25, 0x65, 0x75, 0x08, 0x95, 0x06, 0x81, 0x00, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    let keys = map.decode(&[0x02, 0x00, 0x04, 0, 0, 0, 0, 0]).unwrap();
    assert!(map.application_capabilities(0).unwrap().keyboard);
    assert_eq!(keys.keyboard_modifiers, 0x02);
    assert!(
        keys.controls[..keys.control_count as usize]
            .iter()
            .any(|c| c.usage_page == 7 && c.usage == 4)
    );
    assert_eq!(
        map.decode(&[0, 0, 1, 0, 0, 0, 0, 0]).unwrap_err(),
        HidError::KeyboardRollover
    );
}

#[test]
fn arrays_map_selectors_through_declared_usages() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x09, 0x04, 0x09, 0x07, 0x15, 0x01, 0x25,
        0x02, 0x75, 0x08, 0x95, 0x01, 0x81, 0x00, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    let first = map.decode(&[1]).unwrap();
    let second = map.decode(&[2]).unwrap();
    assert_eq!(first.controls[0].usage_page, 7);
    assert_eq!(first.controls[0].usage, 4);
    assert_eq!(first.controls[0].value, 1);
    assert_eq!(second.controls[0].usage, 7);
    assert_eq!(second.controls[0].value, 2);
}

#[test]
fn button_array_presence_includes_all_declared_usages() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03, 0x15, 0x01, 0x25,
        0x03, 0x75, 0x02, 0x95, 0x02, 0x81, 0x00, 0xc0,
    ];
    let input = HidReportMap::parse(&descriptor)
        .unwrap()
        .decode(&[0x09])
        .unwrap();
    assert_eq!(input.button_usages, 0b011);
    assert_eq!(input.present_button_usages, 0b111);
}

#[test]
fn rejects_all_keyboard_array_error_usages() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, 0x05, 0x07, 0x19, 0x01, 0x29, 0x04, 0x15, 0x01, 0x25,
        0x04, 0x75, 0x08, 0x95, 0x01, 0x81, 0x00, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    for selector in [1, 2, 3] {
        assert_eq!(
            map.decode(&[selector]).unwrap_err(),
            HidError::KeyboardRollover
        );
    }
}

#[test]
fn relative_gamepad_axes_keep_the_raw_delta() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x09, 0x30, 0x15, 0x81, 0x25, 0x7f, 0x75, 0x08, 0x95,
        0x01, 0x81, 0x06, 0xc0,
    ];
    let input = HidReportMap::parse(&descriptor)
        .unwrap()
        .decode(&[0xfb])
        .unwrap();
    assert_eq!(input.axes[0], -5);
    assert_eq!(input.present_axes, 1);
    assert_eq!(input.relative_axes, 1);
}

#[test]
fn output_only_ids_have_no_input_length_or_decode_success() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x01, 0x09, 0x30, 0x15, 0x00, 0x25, 0xff, 0x75,
        0x08, 0x95, 0x01, 0x81, 0x02, 0x85, 0x02, 0x05, 0x08, 0x09, 0x01, 0x91, 0x02, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    assert_eq!(map.required_input_bytes(2), Err(HidError::NoInputReport));
    assert_eq!(
        map.application_capabilities(2),
        Err(HidError::NoInputReport)
    );
    assert_eq!(map.decode(&[2, 0]), Err(HidError::NoInputReport));
}

#[test]
fn decodes_mouse_buttons_and_relative_motion_without_gamepad_axes() {
    let descriptor = [
        0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29,
        0x03, 0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75, 0x05,
        0x81, 0x01, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x15, 0x81, 0x25, 0x7f, 0x75, 0x08, 0x95,
        0x02, 0x81, 0x06, 0xc0, 0xc0,
    ];
    let map = HidReportMap::parse(&descriptor).unwrap();
    let input = map.decode(&[0x05, 0xfb, 0x07]).unwrap();
    assert!(map.application_capabilities(0).unwrap().mouse);
    assert_eq!(input.mouse_buttons, 0b101);
    assert_eq!(input.present_mouse_buttons, 0b111);
    assert_eq!(input.mouse_motion, [-5, 7, 0]);
    assert_eq!(input.present_mouse_motion, 0b011);
    assert_eq!(input.present_axes, 0);
    assert_eq!(input.button_usages, 0);
}

#[test]
fn rejects_truncated_oversize_mixed_id_and_bad_push_pop_descriptors() {
    assert_eq!(
        HidReportMap::parse(&[0x75]).unwrap_err(),
        HidError::TruncatedItem
    );
    assert_eq!(
        HidReportMap::parse(&[0xfe, 0]).unwrap_err(),
        HidError::LongItemsUnsupported
    );
    let oversized = [0x75, 0x20, 0x95, 0x21, 0x81, 0x02];
    assert_eq!(
        HidReportMap::parse(&oversized).unwrap_err(),
        HidError::ReportTooLarge
    );
    assert_eq!(
        HidReportMap::parse(&[0x75, 0x01, 0x95, 0x01, 0x82, 0x02, 0x01]).unwrap_err(),
        HidError::UnsupportedItem
    );
    let mixed = [0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x85, 0x01, 0x81, 0x02];
    assert_eq!(
        HidReportMap::parse(&mixed).unwrap_err(),
        HidError::MixedReportIds
    );
    assert_eq!(
        HidReportMap::parse(&[0xb4]).unwrap_err(),
        HidError::GlobalStackUnderflow
    );
    assert_eq!(
        HidReportMap::parse(&[0xa4]).unwrap_err(),
        HidError::UnbalancedGlobalStack
    );
    assert_eq!(
        HidReportMap::parse(&[0xa4; 9]).unwrap_err(),
        HidError::GlobalStackOverflow
    );
    assert_eq!(
        HidReportMap::parse(&[0xa1, 0x01]).unwrap_err(),
        HidError::UnbalancedCollection
    );
}
