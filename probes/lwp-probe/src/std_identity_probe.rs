//! Measured console settings and chipset revision, with declared board label.
use ogc_system_probe::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.note(&format!(
        "IOS version={} revision={}",
        ogc_ios::get_version(),
        ogc_ios::get_revision()
    ));
    let mut model = [0u8; 48];
    let native = match ogc_conf::model_name(&mut model) {
        Ok(length) => std::str::from_utf8(&model[..length])
            .map(str::to_owned)
            .ok(),
        Err(error) => {
            logger.note_i32("console MODEL read error", error);
            None
        }
    };
    let valid = native.as_ref().is_some_and(|value| {
        !value.is_empty()
            && value.len() < model.len()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    });
    logger.record("console MODEL has bounded printable bytes", valid);
    let native = native.filter(|_| valid);
    let label = |logger: &mut ProbeLogger, key| match crate::std_probe_config::value(key) {
        Ok(Some(value))
            if value.len() <= 40
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_graphic() || byte == b' ') =>
        {
            Some(value)
        }
        Ok(None) => None,
        Ok(Some(_)) => {
            logger.record_with_detail(
                "hardware label option is valid",
                false,
                "label must use 1..40 printable ASCII bytes",
            );
            None
        }
        Err(error) => {
            logger.record_with_detail("hardware label option is valid", false, error);
            None
        }
    };
    let declared = label(logger, "console-model");
    if let Some(native) = &native {
        logger.note(&format!("MODEL from console settings: {native}"));
    }
    let selected = declared
        .as_deref()
        .map(|model| format!("{model} (label)"))
        .or_else(|| native.map(|model| format!("{model} (settings)")))
        .unwrap_or_else(|| "unknown".into());
    let board = label(logger, "motherboard").unwrap_or_else(|| "unknown".into());
    // The production API reads the low-memory Hollywood revision. It does not
    // establish the printed RVL/RVK PCB revision; never infer that label.
    let revision = ogc_system::get_hollywood_revision();
    if let Err(error) = logger.set_hardware_summary(&selected, &board, revision) {
        logger.record_with_detail("hardware summary fits its buffer", false, error);
    }
    logger.note(&format!(
        "Hollywood revision read from hardware: 0x{revision:08x}"
    ));
}
