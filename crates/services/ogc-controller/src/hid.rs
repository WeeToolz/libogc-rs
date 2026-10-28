//! Bounded USB HID report descriptor parser and input report decoder.
//!
//! The parser keeps the usage page and usage for each input value. Applications
//! can bind those usages to actions without assuming that HID Button 1 means A.

pub const MAX_REPORT_IDS: usize = 16;
pub const MAX_INPUT_FIELDS: usize = 128;
pub const MAX_REPORT_BITS: u32 = 1024;
const MAX_LOCAL_USAGES: usize = 128;
const MAX_ARRAY_USAGES: usize = 128;
const MAX_COLLECTION_DEPTH: usize = 8;
const MAX_GLOBAL_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HidError {
    EmptyDescriptor,
    TruncatedItem,
    LongItemsUnsupported,
    UnsupportedItem,
    InvalidItemValue,
    InvalidReportId,
    MixedReportIds,
    TooManyReportIds,
    TooManyFields,
    TooManyUsages,
    CollectionDepth,
    UnbalancedCollection,
    GlobalStackOverflow,
    GlobalStackUnderflow,
    UnbalancedGlobalStack,
    InvalidUsageRange,
    ReportTooLarge,
    NoInputReports,
    NoInputReport,
    UnknownReportId,
    ReportLength,
    InvalidField,
    KeyboardRollover,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidControl {
    pub usage_page: u16,
    pub usage: u16,
    /// Raw logical value. For arrays, this is the selector from the report.
    pub value: i32,
    pub relative: bool,
    pub array: bool,
}

impl HidControl {
    const EMPTY: Self = Self {
        usage_page: 0,
        usage: 0,
        value: 0,
        relative: false,
        array: false,
    };
}

/// Decoded controls from one report. `present_axes` and `present_triggers` say
/// which values this report supplied; absent values do not mean zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidInput {
    pub report_id: u8,
    pub button_usages: u64,
    /// Button usages declared by fields in this report, including released buttons.
    pub present_button_usages: u64,
    pub controls: [HidControl; MAX_INPUT_FIELDS],
    pub control_count: u8,
    /// X, Y, Z, Rx, Ry, Rz mapped from the field's logical range to i16.
    pub axes: [i16; 6],
    pub present_axes: u8,
    /// Bits in `present_axes` whose values are raw relative deltas.
    pub relative_axes: u8,
    /// Generic Desktop Slider and Simulation Accelerator/Brake/Steering.
    pub triggers: [u8; 4],
    pub present_triggers: u8,
    /// Hat value normalized to 0..7. `None` means the null state.
    pub hat: Option<u8>,
    pub hat_present: bool,
    /// D-pad usage bits: up, down, right, left in bits 0..3.
    pub dpad: u8,
    pub dpad_present: u8,
    pub keyboard_modifiers: u8,
    pub keyboard_modifiers_present: u8,
    pub mouse_buttons: u64,
    pub present_mouse_buttons: u64,
    /// Mouse X, Y, Wheel relative values, in that order.
    pub mouse_motion: [i16; 3],
    pub present_mouse_motion: u8,
}

impl HidInput {
    const EMPTY: Self = Self {
        report_id: 0,
        button_usages: 0,
        present_button_usages: 0,
        controls: [HidControl::EMPTY; MAX_INPUT_FIELDS],
        control_count: 0,
        axes: [0; 6],
        present_axes: 0,
        relative_axes: 0,
        triggers: [0; 4],
        present_triggers: 0,
        hat: None,
        hat_present: false,
        dpad: 0,
        dpad_present: 0,
        keyboard_modifiers: 0,
        keyboard_modifiers_present: 0,
        mouse_buttons: 0,
        present_mouse_buttons: 0,
        mouse_motion: [0; 3],
        present_mouse_motion: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReportLayout {
    id: u8,
    input_bits: u32,
    output_bits: u32,
    feature_bits: u32,
}

impl ReportLayout {
    const EMPTY: Self = Self {
        id: 0,
        input_bits: 0,
        output_bits: 0,
        feature_bits: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InputField {
    report_id: u8,
    bit_offset: u32,
    bit_size: u8,
    usage_page: u16,
    usage: u16,
    logical_min: i32,
    logical_max: i32,
    flags: u8,
    application_page: u16,
    application_usage: u16,
    array_page: u16,
    array_usage_start: u8,
    array_usage_count: u8,
    array: bool,
}

impl InputField {
    const EMPTY: Self = Self {
        report_id: 0,
        bit_offset: 0,
        bit_size: 0,
        usage_page: 0,
        usage: 0,
        logical_min: 0,
        logical_max: 0,
        flags: 0,
        application_page: 0,
        application_usage: 0,
        array_page: 0,
        array_usage_start: 0,
        array_usage_count: 0,
        array: false,
    };
}

/// A parsed report layout. All storage is fixed and the value is `Copy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidReportMap {
    reports: [ReportLayout; MAX_REPORT_IDS],
    report_count: u8,
    fields: [InputField; MAX_INPUT_FIELDS],
    field_count: u8,
    array_usages: [u32; MAX_ARRAY_USAGES],
    array_usage_count: u8,
    numbered: bool,
}

/// Application collections present in one report ID.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HidApplicationCapabilities {
    pub keyboard: bool,
    pub mouse: bool,
    pub joystick: bool,
    pub gamepad: bool,
    pub multi_axis: bool,
}

/// Controller fields declared by all controller input reports in a descriptor.
/// Keyboard and mouse application fields do not contribute to this summary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HidControllerCapabilities {
    pub buttons: bool,
    pub left_stick: bool,
    pub right_stick: bool,
    pub triggers: bool,
}

impl HidReportMap {
    /// Parse a HID 1.11 report descriptor with bounded report and item storage.
    pub fn parse(descriptor: &[u8]) -> Result<Self, HidError> {
        if descriptor.is_empty() {
            return Err(HidError::EmptyDescriptor);
        }
        let mut map = Self {
            reports: [ReportLayout::EMPTY; MAX_REPORT_IDS],
            report_count: 0,
            fields: [InputField::EMPTY; MAX_INPUT_FIELDS],
            field_count: 0,
            array_usages: [0; MAX_ARRAY_USAGES],
            array_usage_count: 0,
            numbered: false,
        };
        let mut globals = Globals::EMPTY;
        let mut global_stack = [Globals::EMPTY; MAX_GLOBAL_DEPTH];
        let mut global_depth = 0usize;
        let mut locals = Locals::EMPTY;
        let mut collections = [Collection::EMPTY; MAX_COLLECTION_DEPTH];
        let mut collection_depth = 0usize;
        let mut offset_by_type = [[0u32; 3]; MAX_REPORT_IDS];

        let mut pos = 0usize;
        while pos < descriptor.len() {
            let prefix = descriptor[pos];
            pos += 1;
            if prefix == 0xfe {
                return Err(HidError::LongItemsUnsupported);
            }
            let size = match prefix & 0x03 {
                0 => 0,
                1 => 1,
                2 => 2,
                _ => 4,
            };
            if descriptor.len() - pos < size {
                return Err(HidError::TruncatedItem);
            }
            let raw = read_le(&descriptor[pos..pos + size]);
            pos += size;
            let item_type = (prefix >> 2) & 0x03;
            let tag = prefix >> 4;
            match item_type {
                0 => match tag {
                    8 | 9 | 11 => {
                        if raw & !0x1ff != 0 {
                            return Err(HidError::InvalidItemValue);
                        }
                        if tag == 8 && raw & 0x180 != 0 {
                            // Input bit 7 is reserved; bit 8 selects Buffered Bytes,
                            // which needs byte-stream semantics this map does not model.
                            return Err(HidError::UnsupportedItem);
                        }
                        globals.main_flags = raw;
                        let report_type = match tag {
                            8 => 0,
                            9 => 1,
                            _ => 2,
                        };
                        let report_index = map.report_index(globals.report_id)?;
                        let bits = globals
                            .report_size
                            .checked_mul(globals.report_count)
                            .ok_or(HidError::ReportTooLarge)?;
                        let end = offset_by_type[report_index][report_type]
                            .checked_add(bits)
                            .ok_or(HidError::ReportTooLarge)?;
                        if end > MAX_REPORT_BITS {
                            return Err(HidError::ReportTooLarge);
                        }
                        offset_by_type[report_index][report_type] = end;
                        match tag {
                            8 => map.add_input_fields(
                                globals,
                                locals,
                                collections,
                                collection_depth,
                                offset_by_type[report_index][0] - bits,
                            )?,
                            9 | 11 => { /* Layout offsets are still tracked and bounded. */ }
                            _ => return Err(HidError::UnsupportedItem),
                        }
                        map.reports[report_index].input_bits = offset_by_type[report_index][0];
                        map.reports[report_index].output_bits = offset_by_type[report_index][1];
                        map.reports[report_index].feature_bits = offset_by_type[report_index][2];
                        locals = Locals::EMPTY;
                    }
                    10 => {
                        if raw > u8::MAX as u32 {
                            return Err(HidError::InvalidItemValue);
                        }
                        if collection_depth == MAX_COLLECTION_DEPTH {
                            return Err(HidError::CollectionDepth);
                        }
                        let (page, usage) = locals.first_usage(globals.usage_page);
                        let app = if collection_depth == 0 || raw as u8 == 1 {
                            (page, usage)
                        } else {
                            collections[collection_depth - 1].application
                        };
                        collections[collection_depth] = Collection { application: app };
                        collection_depth += 1;
                        locals = Locals::EMPTY;
                    }
                    12 => {
                        if collection_depth == 0 {
                            return Err(HidError::UnbalancedCollection);
                        }
                        collection_depth -= 1;
                        locals = Locals::EMPTY;
                    }
                    _ => return Err(HidError::UnsupportedItem),
                },
                1 => match tag {
                    0 => {
                        globals.usage_page =
                            u16::try_from(raw).map_err(|_| HidError::InvalidItemValue)?
                    }
                    1 => globals.logical_min = signed_value(raw, size),
                    2 => {
                        globals.logical_max = if globals.logical_min < 0 {
                            signed_value(raw, size)
                        } else {
                            i32::try_from(raw).map_err(|_| HidError::InvalidItemValue)?
                        }
                    }
                    3..=6 => { /* Physical range, unit exponent, and unit do not change input decoding. */
                    }
                    7 => globals.report_size = raw,
                    8 => {
                        if raw == 0 || raw > u8::MAX as u32 {
                            return Err(HidError::InvalidReportId);
                        }
                        if map.reports[..map.report_count as usize]
                            .iter()
                            .any(|r| r.id == 0)
                        {
                            return Err(HidError::MixedReportIds);
                        }
                        globals.report_id = raw as u8;
                        map.numbered = true;
                    }
                    9 => globals.report_count = raw,
                    10 => {
                        if global_depth == MAX_GLOBAL_DEPTH {
                            return Err(HidError::GlobalStackOverflow);
                        }
                        global_stack[global_depth] = globals;
                        global_depth += 1;
                    }
                    11 => {
                        if global_depth == 0 {
                            return Err(HidError::GlobalStackUnderflow);
                        }
                        global_depth -= 1;
                        globals = global_stack[global_depth];
                    }
                    _ => return Err(HidError::UnsupportedItem),
                },
                2 => match tag {
                    0 => locals.add_usage(expand_usage(raw, size, globals.usage_page)?)?,
                    1 => locals.usage_min = Some(expand_usage(raw, size, globals.usage_page)?),
                    2 => locals.usage_max = Some(expand_usage(raw, size, globals.usage_page)?),
                    _ => return Err(HidError::UnsupportedItem),
                },
                _ => return Err(HidError::UnsupportedItem),
            }
        }
        if collection_depth != 0 {
            return Err(HidError::UnbalancedCollection);
        }
        if global_depth != 0 {
            return Err(HidError::UnbalancedGlobalStack);
        }
        if !map.reports[..map.report_count as usize]
            .iter()
            .any(|r| r.input_bits != 0)
        {
            return Err(HidError::NoInputReports);
        }
        Ok(map)
    }

    /// Decode one complete Input report. Numbered reports include their ID byte.
    pub fn decode(&self, bytes: &[u8]) -> Result<HidInput, HidError> {
        let (report_id, payload) = if self.numbered {
            let id = *bytes.first().ok_or(HidError::ReportLength)?;
            (id, &bytes[1..])
        } else {
            (0, bytes)
        };
        let report = self
            .find_report(report_id)
            .ok_or(HidError::UnknownReportId)?;
        if report.input_bits == 0 {
            return Err(HidError::NoInputReport);
        }
        let expected = report.input_bits.div_ceil(8) as usize;
        if payload.len() != expected {
            return Err(HidError::ReportLength);
        }
        let mut input = HidInput::EMPTY;
        input.report_id = report_id;
        for field in self.fields[..self.field_count as usize]
            .iter()
            .filter(|f| f.report_id == report_id)
        {
            if field.flags & 1 != 0 {
                continue;
            }
            let raw = extract_bits(payload, field.bit_offset, field.bit_size)?;
            let value = decode_logical(raw, field.bit_size, field.logical_min);
            let outside_range = value < field.logical_min || value > field.logical_max;
            if outside_range && field.flags & 0x40 == 0 {
                return Err(HidError::InvalidField);
            }
            let (page, usage) = if field.array {
                if outside_range {
                    (field.array_page, 0)
                } else {
                    let index = usize::try_from(value as i64 - field.logical_min as i64)
                        .map_err(|_| HidError::InvalidField)?;
                    let start = field.array_usage_start as usize;
                    let end = start + field.array_usage_count as usize;
                    if index >= field.array_usage_count as usize
                        || end > self.array_usage_count as usize
                    {
                        return Err(HidError::InvalidField);
                    }
                    (field.array_page, self.array_usages[start + index] as u16)
                }
            } else {
                (field.usage_page, field.usage)
            };
            if input.control_count as usize == MAX_INPUT_FIELDS {
                return Err(HidError::TooManyFields);
            }
            input.controls[input.control_count as usize] = HidControl {
                usage_page: page,
                usage,
                value,
                relative: field.flags & 4 != 0,
                array: field.array,
            };
            input.control_count += 1;
            self.map_control(&mut input, field, page, usage, value)?;
        }
        Ok(input)
    }

    /// Return the complete input report length, including the ID prefix when used.
    pub fn required_input_bytes(&self, report_id: u8) -> Result<usize, HidError> {
        let report = self
            .find_report(report_id)
            .ok_or(HidError::UnknownReportId)?;
        if report.input_bits == 0 {
            return Err(HidError::NoInputReport);
        }
        Ok(report.input_bits.div_ceil(8) as usize + usize::from(self.numbered))
    }

    /// Return the largest complete input report length, including any ID prefix.
    pub fn maximum_input_bytes(&self) -> usize {
        self.reports[..self.report_count as usize]
            .iter()
            .filter(|report| report.input_bits != 0)
            .map(|report| report.input_bits.div_ceil(8) as usize + usize::from(self.numbered))
            .max()
            .unwrap_or(0)
    }

    /// Summarize controller controls across every report ID in this descriptor.
    /// This is stable when a device sends axes and buttons in separate reports.
    pub fn game_controller_capabilities(&self) -> HidControllerCapabilities {
        let mut axes = 0u8;
        let mut capabilities = HidControllerCapabilities::default();
        for field in self.fields[..self.field_count as usize]
            .iter()
            .filter(|field| {
                field.flags & 1 == 0
                    && field.application_page == 1
                    && matches!(field.application_usage, 4 | 5 | 8)
            })
        {
            if field.usage_page == 1 && (0x30..=0x35).contains(&field.usage) {
                axes |= 1 << (field.usage - 0x30);
            }
            capabilities.buttons |= (field.usage_page == 9 && !field.array)
                || (field.array && field.array_page == 9)
                || (field.usage_page == 1
                    && (field.usage == 0x39 || (0x90..=0x93).contains(&field.usage)));
            capabilities.triggers |= (field.usage_page == 1 && field.usage == 0x36)
                || (field.usage_page == 2 && matches!(field.usage, 0xc4 | 0xc5 | 0xc8));
        }
        capabilities.left_stick = axes & 0b0000_0011 == 0b0000_0011;
        capabilities.right_stick = axes & 0b0001_1000 == 0b0001_1000;
        capabilities
    }

    /// Return true when any input report belongs to a joystick, gamepad, or
    /// multi-axis controller application. Keyboard and mouse collections do
    /// not qualify as controller input.
    pub fn has_game_controller_application(&self) -> bool {
        self.fields[..self.field_count as usize]
            .iter()
            .any(|field| {
                field.flags & 1 == 0
                    && field.application_page == 1
                    && matches!(field.application_usage, 4 | 5 | 8)
            })
    }

    /// Return true when this packet's report ID belongs to a controller
    /// application. Mixed keyboard/controller interfaces can then ignore
    /// unrelated reports without interpreting them as controller input.
    pub fn is_game_controller_report(&self, bytes: &[u8]) -> Result<bool, HidError> {
        let report_id = if self.numbered {
            *bytes.first().ok_or(HidError::ReportLength)?
        } else {
            0
        };
        let capabilities = self.application_capabilities(report_id)?;
        Ok(capabilities.joystick || capabilities.gamepad || capabilities.multi_axis)
    }

    /// Report which application types use this report ID.
    pub fn application_capabilities(
        &self,
        report_id: u8,
    ) -> Result<HidApplicationCapabilities, HidError> {
        let report = self
            .find_report(report_id)
            .ok_or(HidError::UnknownReportId)?;
        if report.input_bits == 0 {
            return Err(HidError::NoInputReport);
        }
        let mut capabilities = HidApplicationCapabilities::default();
        for field in self.fields[..self.field_count as usize]
            .iter()
            .filter(|field| field.report_id == report_id && field.flags & 1 == 0)
        {
            if field.application_page != 1 {
                continue;
            }
            match field.application_usage {
                2 => capabilities.mouse = true,
                4 => capabilities.joystick = true,
                5 => capabilities.gamepad = true,
                6 => capabilities.keyboard = true,
                8 => capabilities.multi_axis = true,
                _ => {}
            }
        }
        Ok(capabilities)
    }

    fn report_index(&mut self, id: u8) -> Result<usize, HidError> {
        if self.numbered && id == 0 {
            return Err(HidError::MixedReportIds);
        }
        if !self.numbered && id != 0 {
            return Err(HidError::MixedReportIds);
        }
        if let Some(i) = self.reports[..self.report_count as usize]
            .iter()
            .position(|r| r.id == id)
        {
            return Ok(i);
        }
        if self.report_count as usize == MAX_REPORT_IDS {
            return Err(HidError::TooManyReportIds);
        }
        let i = self.report_count as usize;
        self.reports[i] = ReportLayout {
            id,
            ..ReportLayout::EMPTY
        };
        self.report_count += 1;
        Ok(i)
    }

    fn find_report(&self, id: u8) -> Option<&ReportLayout> {
        self.reports[..self.report_count as usize]
            .iter()
            .find(|r| r.id == id)
    }

    fn add_input_fields(
        &mut self,
        g: Globals,
        l: Locals,
        c: [Collection; MAX_COLLECTION_DEPTH],
        depth: usize,
        base: u32,
    ) -> Result<(), HidError> {
        if g.report_size == 0 || g.report_size > 32 || g.report_count == 0 {
            return Err(HidError::InvalidItemValue);
        }
        if g.logical_min > g.logical_max {
            return Err(HidError::InvalidItemValue);
        }
        let (usages, usage_count) = l.expanded_usages(g.usage_page)?;
        let (app_page, app_usage) = if depth == 0 {
            (0, 0)
        } else {
            c[depth - 1].application
        };
        let array = g.main_flags & 1 == 0 && g.main_flags & 2 == 0;
        if array && usage_count == 0 {
            return Err(HidError::UnsupportedItem);
        }
        if array
            && usage_count != 0
            && usages[..usage_count]
                .iter()
                .any(|usage| usage >> 16 != usages[0] >> 16)
        {
            return Err(HidError::UnsupportedItem);
        }
        let array_usage_start = self.array_usage_count as usize;
        if array && array_usage_start + usage_count > MAX_ARRAY_USAGES {
            return Err(HidError::TooManyUsages);
        }
        if array {
            self.array_usages[array_usage_start..array_usage_start + usage_count]
                .copy_from_slice(&usages[..usage_count]);
            self.array_usage_count += usage_count as u8;
        }
        for i in 0..g.report_count as usize {
            if self.field_count as usize == MAX_INPUT_FIELDS {
                return Err(HidError::TooManyFields);
            }
            let usage = if array || usage_count == 0 {
                0
            } else {
                usages[core::cmp::min(i, usage_count - 1)]
            };
            let offset = base
                .checked_add(
                    (i as u32)
                        .checked_mul(g.report_size)
                        .ok_or(HidError::ReportTooLarge)?,
                )
                .ok_or(HidError::ReportTooLarge)?;
            self.fields[self.field_count as usize] = InputField {
                report_id: g.report_id,
                bit_offset: offset,
                bit_size: g.report_size as u8,
                usage_page: (usage >> 16) as u16,
                usage: usage as u16,
                logical_min: g.logical_min,
                logical_max: g.logical_max,
                flags: g.main_flags as u8,
                application_page: app_page,
                application_usage: app_usage,
                array_page: if usage_count == 0 {
                    g.usage_page
                } else {
                    (usages[0] >> 16) as u16
                },
                array_usage_start: array_usage_start as u8,
                array_usage_count: usage_count as u8,
                array,
            };
            self.field_count += 1;
        }
        Ok(())
    }

    fn map_control(
        &self,
        out: &mut HidInput,
        f: &InputField,
        page: u16,
        usage: u16,
        value: i32,
    ) -> Result<(), HidError> {
        let app = f.application_page == 1;
        let active = if f.array { usage != 0 } else { value != 0 };
        if f.array && page == 9 {
            let start = f.array_usage_start as usize;
            let end = start + f.array_usage_count as usize;
            for declared in self.array_usages[start..end].iter() {
                let declared = *declared as u16;
                if (1..=64).contains(&declared) {
                    let mask = 1u64 << (declared - 1);
                    if app && f.application_usage == 2 {
                        out.present_mouse_buttons |= mask;
                    } else {
                        out.present_button_usages |= mask;
                    }
                }
            }
        }
        if page == 9 && usage > 0 && usage <= 64 {
            let mask = 1u64 << (usage - 1);
            if app && f.application_usage == 2 {
                out.present_mouse_buttons |= mask;
                if active {
                    out.mouse_buttons |= mask;
                }
            } else {
                out.present_button_usages |= mask;
                if active {
                    out.button_usages |= mask;
                }
            }
        }
        if page == 7 && app && f.application_usage == 6 {
            if f.array {
                if matches!(usage, 1..=3) {
                    return Err(HidError::KeyboardRollover);
                }
            } else if (0xe0..=0xe7).contains(&usage) {
                let mask = 1 << (usage - 0xe0);
                out.keyboard_modifiers_present |= mask;
                if active {
                    out.keyboard_modifiers |= mask;
                }
            }
        }
        if page == 1 && app && f.application_usage == 2 && f.flags & 4 != 0 {
            let slot = match usage {
                0x30 => Some(0),
                0x31 => Some(1),
                0x38 => Some(2),
                _ => None,
            };
            if let Some(slot) = slot {
                out.mouse_motion[slot] =
                    i16::try_from(value).map_err(|_| HidError::InvalidField)?;
                out.present_mouse_motion |= 1 << slot;
            }
        }
        let game_collection = f.application_page == 1 && matches!(f.application_usage, 4 | 5 | 8);
        if page == 1 && game_collection && !f.array {
            if usage == 0x39 {
                out.hat_present = true;
                if value < f.logical_min || value > f.logical_max {
                    out.hat = None;
                } else {
                    let n = value - f.logical_min;
                    out.hat = if (0..=7).contains(&n) {
                        Some(n as u8)
                    } else {
                        return Err(HidError::InvalidField);
                    };
                }
            }
            if (0x30..=0x35).contains(&usage) {
                let slot = (usage - 0x30) as usize;
                if f.flags & 4 != 0 {
                    out.axes[slot] = i16::try_from(value).map_err(|_| HidError::InvalidField)?;
                    out.relative_axes |= 1 << slot;
                } else {
                    out.axes[slot] = normalize(value, f.logical_min, f.logical_max)?;
                }
                out.present_axes |= 1 << slot;
            }
            if usage == 0x36 {
                out.triggers[0] = normalize_u8(value, f.logical_min, f.logical_max)?;
                out.present_triggers |= 1;
            }
            if (0x90..=0x93).contains(&usage) {
                let mask = 1 << (usage - 0x90);
                out.dpad_present |= mask;
                if active {
                    out.dpad |= mask;
                }
            }
        }
        if page == 2 && game_collection && usage == 0xc4 {
            out.triggers[1] = normalize_u8(value, f.logical_min, f.logical_max)?;
            out.present_triggers |= 2;
        } else if page == 2 && game_collection && usage == 0xc5 {
            out.triggers[2] = normalize_u8(value, f.logical_min, f.logical_max)?;
            out.present_triggers |= 4;
        } else if page == 2 && game_collection && usage == 0xc8 {
            out.triggers[3] = normalize_u8(value, f.logical_min, f.logical_max)?;
            out.present_triggers |= 8;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Globals {
    usage_page: u16,
    logical_min: i32,
    logical_max: i32,
    report_size: u32,
    report_count: u32,
    report_id: u8,
    main_flags: u32,
}
impl Globals {
    const EMPTY: Self = Self {
        usage_page: 0,
        logical_min: 0,
        logical_max: 0,
        report_size: 0,
        report_count: 0,
        report_id: 0,
        main_flags: 0,
    };
}

#[derive(Clone, Copy)]
struct Collection {
    application: (u16, u16),
}
impl Collection {
    const EMPTY: Self = Self {
        application: (0, 0),
    };
}

#[derive(Clone, Copy)]
struct Locals {
    usages: [u32; MAX_LOCAL_USAGES],
    count: usize,
    usage_min: Option<u32>,
    usage_max: Option<u32>,
}
impl Locals {
    const EMPTY: Self = Self {
        usages: [0; MAX_LOCAL_USAGES],
        count: 0,
        usage_min: None,
        usage_max: None,
    };
    fn add_usage(&mut self, usage: u32) -> Result<(), HidError> {
        if self.count == MAX_LOCAL_USAGES {
            return Err(HidError::TooManyUsages);
        }
        self.usages[self.count] = usage;
        self.count += 1;
        Ok(())
    }
    fn first_usage(&self, default_page: u16) -> (u16, u16) {
        if self.count != 0 {
            ((self.usages[0] >> 16) as u16, self.usages[0] as u16)
        } else if let (Some(min), Some(_)) = (self.usage_min, self.usage_max) {
            ((min >> 16) as u16, min as u16)
        } else {
            (default_page, 0)
        }
    }
    fn expanded_usages(&self, page: u16) -> Result<([u32; MAX_LOCAL_USAGES], usize), HidError> {
        let mut values = self.usages;
        let mut count = self.count;
        match (self.usage_min, self.usage_max) {
            (Some(min), Some(max)) => {
                if min >> 16 != max >> 16 || max < min {
                    return Err(HidError::InvalidUsageRange);
                }
                let n = max as u64 - min as u64 + 1;
                if n > (MAX_LOCAL_USAGES - count) as u64 {
                    return Err(HidError::TooManyUsages);
                }
                for value in min..=max {
                    values[count] = value;
                    count += 1;
                }
            }
            (None, None) => {}
            _ => return Err(HidError::InvalidUsageRange),
        }
        if count == 0 {
            let _ = page;
        }
        Ok((values, count))
    }
}

fn read_le(data: &[u8]) -> u32 {
    let mut value = 0u32;
    for (i, byte) in data.iter().enumerate() {
        value |= (*byte as u32) << (i * 8);
    }
    value
}
fn signed_value(raw: u32, size: usize) -> i32 {
    if size == 0 {
        0
    } else if size == 4 {
        raw as i32
    } else {
        let shift = 32 - size as u32 * 8;
        ((raw << shift) as i32) >> shift
    }
}
fn expand_usage(raw: u32, size: usize, page: u16) -> Result<u32, HidError> {
    if size == 4 {
        Ok(raw)
    } else if raw > u16::MAX as u32 {
        Err(HidError::InvalidItemValue)
    } else {
        Ok(((page as u32) << 16) | raw)
    }
}
fn decode_logical(raw: u32, size: u8, min: i32) -> i32 {
    if min < 0 {
        signed_bits(raw, size)
    } else {
        raw as i32
    }
}
fn signed_bits(raw: u32, width: u8) -> i32 {
    if width == 0 || width >= 32 {
        raw as i32
    } else {
        let sign = 1u32 << (width - 1);
        if raw & sign == 0 {
            raw as i32
        } else {
            (raw | (!0u32 << width)) as i32
        }
    }
}
fn extract_bits(bytes: &[u8], offset: u32, width: u8) -> Result<u32, HidError> {
    if width == 0
        || width > 32
        || offset
            .checked_add(width as u32)
            .ok_or(HidError::InvalidField)?
            > (bytes.len() as u32) * 8
    {
        return Err(HidError::ReportLength);
    }
    let mut value = 0u32;
    for bit in 0..width as u32 {
        let source = offset + bit;
        if bytes[(source / 8) as usize] & (1 << (source % 8)) != 0 {
            value |= 1 << bit;
        }
    }
    Ok(value)
}
fn normalize(value: i32, min: i32, max: i32) -> Result<i16, HidError> {
    if min >= max || value < min || value > max {
        return Err(HidError::InvalidField);
    }
    let scaled = ((value as i64 - min as i64) * 65535) / (max as i64 - min as i64) - 32768;
    i16::try_from(scaled).map_err(|_| HidError::InvalidField)
}
fn normalize_u8(value: i32, min: i32, max: i32) -> Result<u8, HidError> {
    if min >= max || value < min || value > max {
        return Err(HidError::InvalidField);
    }
    u8::try_from(((value as i64 - min as i64) * 255) / (max as i64 - min as i64))
        .map_err(|_| HidError::InvalidField)
}
