//! GX register values shared by solid and textured 2D drawing.

pub(crate) const TEXTURE_GENERATOR_COUNT: u32 = 1;

pub(crate) fn vertex_spec() -> u32 {
    0x11 // HBC C submits color 0 and TEX0 for solid and textured vertices.
}

pub(crate) fn solid_generation_mode() -> u32 {
    // libogc stores channel and texgen counts directly. One TEV stage uses encoded value zero.
    (1 << 4) | TEXTURE_GENERATOR_COUNT
}

pub(crate) fn tev_order_word() -> u32 {
    // Match libogc GX_Init / HBC gfx.c: stage 0 selects texture coordinate
    // 0, texture map 0, and rasterized color 0. Solid TEV inputs ignore the
    // texture sample, but preserving the default order keeps GX state valid.
    1 << 6
}

pub(crate) fn textured_texgen_words() -> [u32; 2] {
    // GX_TG_MTX2x4, sourced from TEX0, with the identity texture matrix.
    [0x280, 0x3d]
}

pub(crate) fn pass_color_tev_words() -> [u32; 2] {
    // RASC/ RASA pass-through to TEVPREV, with clamp and the default add operation.
    [0xc008_fffa, 0xc108_ffd0]
}

pub(crate) fn matrix_index_word() -> u32 {
    60 << 6 // GX_IDENTITY selected by HBC C for TEXCOORD0.
}

pub(crate) fn identity_swap_words() -> [u32; 2] {
    // GX_SetTevSwapModeTable(GX_TEV_SWAP0, RED, GREEN, BLUE, ALPHA).
    [0xf600_0004, 0xf700_000e]
}

// XF post-transform matrix selected by GX_DTTIDENTITY.
pub(crate) const POST_IDENTITY_ADDRESS: u16 = 0x0500 + (125 - 64) * 4;

pub(crate) fn texture_map_registers(map_id: u8) -> Option<[u8; 6]> {
    if map_id >= 8 {
        return None;
    }
    let slot = map_id & 3;
    let base = if map_id < 4 { 0x80 } else { 0xa0 };
    Some([
        base + slot,
        base + 4 + slot,
        base + 8 + slot,
        base + 12 + slot,
        base + 16 + slot,
        base + 20 + slot,
    ])
}

pub(crate) fn texture_coord_scale_words(
    coord: u8,
    width: u16,
    height: u16,
    filter: u32,
) -> [u32; 2] {
    // __SetSURegs uses dimensions minus one and the S/T wrap bits from the texture filter word.
    let reg_s = 0x30 + u32::from(coord) * 2;
    let reg_t = reg_s + 1;
    [
        (reg_s << 24) | u32::from(width - 1) | ((filter & 1) << 16),
        (reg_t << 24) | u32::from(height - 1) | (((filter >> 2) & 1) << 16),
    ]
}

pub(crate) fn valid_copy_width(width: u16, max_width: u16) -> bool {
    width != 0 && width <= max_width && width.is_multiple_of(16)
}

pub(crate) fn copy_pixel_engine_word(configured: u32, disable_early_z: bool) -> u32 {
    if disable_early_z {
        configured & !0x40
    } else {
        configured
    }
}

pub(crate) fn texture_lod_bias_field(lodbias: f32) -> u32 {
    let lodbias = if lodbias < -4.0 {
        -4.0
    } else if lodbias == 4.0 {
        3.99
    } else {
        lodbias
    };
    (((32.0 * lodbias) as i32 as u32) & 0xff) << 9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_pass_color_does_not_read_previous_tev_output() {
        // GX_CC_ZERO is 15. GX_CA_ZERO is 7. Numeric zero selects PREV.
        let [color, alpha] = pass_color_tev_words();
        assert_eq!(tev_order_word(), 0x40); // Keep GX_COLOR0A0 selected for raster color.
        for shift in [12, 8, 4] {
            assert_eq!((color >> shift) & 15, 15);
        }
        for shift in [13, 10, 7] {
            assert_eq!((alpha >> shift) & 7, 7);
        }
        assert_eq!(color & 15, 10); // RASC
        assert_eq!((alpha >> 4) & 7, 5); // RASA
    }

    #[test]
    fn solid_and_textured_vertices_keep_the_c_texcoord_stream_enabled() {
        assert_eq!(solid_generation_mode(), 0x11);
        assert_eq!(TEXTURE_GENERATOR_COUNT, 1);
        assert_eq!(vertex_spec(), 0x11);
        assert_eq!(solid_generation_mode() & 15, TEXTURE_GENERATOR_COUNT);
        assert_eq!((vertex_spec() >> 4) & 15, TEXTURE_GENERATOR_COUNT);
        assert_eq!(solid_generation_mode() >> 4 & 7, 1);
        assert_eq!(vertex_spec() & 3, 1);
    }

    #[test]
    fn texture_transform_uses_two_components_and_identity_matrix() {
        let [texgen, post_matrix] = textured_texgen_words();
        assert_eq!(texgen & 2, 0); // GX_TG_MTX2x4, not 3x4.
        assert_eq!(texgen >> 7 & 31, 5); // TEX0 source row.
        assert_eq!(matrix_index_word() >> 6 & 63, 60); // GX_IDENTITY.
        assert_eq!(POST_IDENTITY_ADDRESS, 0x05f4);
        assert_eq!(post_matrix, 125 - 64); // GX_DTTIDENTITY - GX_DTTMTX0.
    }

    #[test]
    fn texture_map_registers_follow_libogc_tables() {
        assert_eq!(
            texture_map_registers(0),
            Some([0x80, 0x84, 0x88, 0x8c, 0x90, 0x94])
        );
        assert_eq!(
            texture_map_registers(3),
            Some([0x83, 0x87, 0x8b, 0x8f, 0x93, 0x97])
        );
        assert_eq!(
            texture_map_registers(4),
            Some([0xa0, 0xa4, 0xa8, 0xac, 0xb0, 0xb4])
        );
        assert_eq!(
            texture_map_registers(7),
            Some([0xa3, 0xa7, 0xab, 0xaf, 0xb3, 0xb7])
        );
        assert_eq!(texture_map_registers(8), None);
    }

    #[test]
    fn texture_dirty_flush_words_match_libogc_su_size_registers() {
        assert_eq!(
            texture_coord_scale_words(0, 640, 480, 0),
            [0x3000_027f, 0x3100_01df]
        );
        assert_eq!(
            texture_coord_scale_words(0, 128, 48, 5),
            [0x3001_007f, 0x3101_002f]
        );
    }

    #[test]
    fn copy_width_must_fit_the_destination_register() {
        assert!(valid_copy_width(640, 1024));
        assert!(!valid_copy_width(641, 1024));
        assert!(!valid_copy_width(0, 1024));
        assert!(!valid_copy_width(1040, 1024));
    }

    #[test]
    fn clearing_copy_disables_and_then_restores_z_compare_location() {
        assert_eq!(copy_pixel_engine_word(0x40, true), 0);
        assert_eq!(copy_pixel_engine_word(0x40, false), 0x40);
        assert_eq!(copy_pixel_engine_word(0x47, true), 0x07);
    }

    #[test]
    fn texture_lod_bias_keeps_signed_low_bits() {
        assert_eq!(texture_lod_bias_field(-1.0), 0x1c000);
        assert_eq!(texture_lod_bias_field(-4.0), 0x10000);
        assert_eq!(texture_lod_bias_field(1.0), 0x4000);
        assert_eq!(texture_lod_bias_field(4.0), 0xfe00);
    }

    #[test]
    fn swap_zero_preserves_all_four_color_components() {
        let [rg, ba] = identity_swap_words();
        assert_eq!([rg & 3, rg >> 2 & 3, ba & 3, ba >> 2 & 3], [0, 1, 2, 3]);
    }
}
