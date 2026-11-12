// SPDX-License-Identifier: GPL-2.0-only
// Copyright (c) 2026 Stela contributors
//! Exercise the physical driver's checked MMIO view, not the protocol model.
use super::*;

fn mapped_handoff(memory: &mut [u32]) -> (Handoff, Option<pci::Bar>) {
    let base = memory.as_mut_ptr() as u64;
    let bytes = core::mem::size_of_val(memory) as u64;
    (
        Handoff {
            ecam: vec![],
            mapped: core::iter::once(base..base + bytes).collect(),
            dma: 0..0,
            clock: crate::Clock {
                ticks_per_microsecond: 1,
            },
            firmware: None,
            completion: None,
        },
        Some(pci::Bar::Memory {
            address: base,
            size: bytes,
            prefetchable: false,
            wide: true,
        }),
    )
}

#[test]
fn physical_nbase_view_retains_the_validated_aperture_for_station_and_mac_registers() {
    for xid in [0x609, 0x641, 0x688, 0x708, 0x681, 0x649] {
        let mut memory = vec![0u32; (NBASE_REGISTER_BYTES as usize).div_ceil(4)];
        memory[NBASE_MAC0_BACKUP / 4] = 0x0504_0302;
        memory[(NBASE_MAC0_BACKUP + 4) / 4] = 0x0706;
        let (handoff, bar) = mapped_handoff(&mut memory);
        let initial = map_registers(&handoff, bar, 0, 256).unwrap();
        let (device, revision) = mapped_revision(initial, xid << 20, |bytes| {
            map_registers(&handoff, bar, 0, bytes)
        })
        .unwrap();
        // Match the observed boot path: first reset is within 256 bytes;
        // reading the backed-up station address then reaches 0x19e0.
        let time = Time::default();
        let hardware = Hardware::new(&device, &time, revision, 0);
        assert_eq!(read_station(&hardware).unwrap(), Mac([2, 3, 4, 5, 6, 7]));
        assert!(device.len() >= NBASE_REGISTER_BYTES as usize);
        device.write16(NBASE_EEE_TXIDLE, 0x1234);
        assert_eq!(device.read16(NBASE_EEE_TXIDLE), 0x1234);
    }
}

#[test]
fn physical_extended_xid_is_read_only_after_its_view_is_expanded() {
    let mut memory = vec![0u32; (EXTENDED_REGISTER_BYTES as usize).div_ceil(4)];
    let (handoff, bar) = mapped_handoff(&mut memory);
    let initial = map_registers(&handoff, bar, 0, 256).unwrap();
    let (device, revision) = mapped_revision(initial, 0x7c8 << 20, |bytes| {
        map_registers(&handoff, bar, 0, bytes)
    })
    .unwrap();
    assert_eq!(revision.extended_xid, Some(0));
    assert!(device.len() >= EXTENDED_REGISTER_BYTES as usize);
}

#[test]
fn physical_mapping_rejects_short_bars_and_keeps_legacy_views_small() {
    let mut memory = vec![0u32; 64];
    let (handoff, bar) = mapped_handoff(&mut memory);
    let initial = map_registers(&handoff, bar, 0, 256).unwrap();
    for xid in [0x688, 0x7c8] {
        assert!(matches!(
            mapped_revision(initial, xid << 20, |bytes| {
                map_registers(&handoff, bar, 0, bytes)
            }),
            Err(Error::Device(BlockError::Unsupported(_)))
        ));
    }
    let (device, revision) = mapped_revision(initial, 0x380 << 20, |_| {
        panic!("legacy device must not request an extended mapping")
    })
    .unwrap();
    assert_eq!(revision.version, 17);
    assert_eq!(device.len(), 256);

    let mut memory = vec![0u32; (NBASE_REGISTER_BYTES as usize).div_ceil(4)];
    let (mut handoff, bar) = mapped_handoff(&mut memory);
    handoff.mapped[0].end = handoff.mapped[0].start + 256;
    let initial = map_registers(&handoff, bar, 0, 256).unwrap();
    assert!(matches!(
        mapped_revision(initial, 0x688 << 20, |bytes| {
            map_registers(&handoff, bar, 0, bytes)
        }),
        Err(Error::Device(BlockError::Unsupported(
            "registers are not mapped"
        )))
    ));
}
