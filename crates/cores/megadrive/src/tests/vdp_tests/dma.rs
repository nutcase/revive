use super::*;

#[test]
fn complete_bus_dma_updates_source_and_clears_length_registers() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0x8150); // display + DMA enable
    vdp.write_control_port(0x9302); // length low
    vdp.write_control_port(0x9400); // length high
    vdp.write_control_port(0x9500); // source low
    vdp.write_control_port(0x9600); // source mid
    vdp.write_control_port(0x9700); // source high / bus mode

    // Queue bus DMA request.
    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0080);
    let _ = vdp.take_bus_dma_request().expect("request expected");

    vdp.complete_bus_dma(0x0012_3456);
    // DMA length should be cleared.
    assert_eq!(vdp.register(19), 0);
    assert_eq!(vdp.register(20), 0);

    // Source LOW/MID should encode 0x123456 >> 1 = 0x091A2B (low=0x2B, mid=0x1A).
    assert_eq!(vdp.register(21), 0x2B);
    assert_eq!(vdp.register(22), 0x1A);
    // Source HIGH should NOT be updated (frozen during transfer).
    assert_eq!(
        vdp.register(23) & 0x7F,
        0x00,
        "DMA source high register should be frozen"
    );
}

#[test]
fn complete_bus_dma_freezes_source_high_register() {
    let mut vdp = Vdp::new();
    vdp.write_control_port(0x8150); // display + DMA enable
    vdp.write_control_port(0x9302); // length low
    vdp.write_control_port(0x9400); // length high
    vdp.write_control_port(0x9500); // source low = 0
    vdp.write_control_port(0x9600); // source mid = 0
    vdp.write_control_port(0x9710); // source high = 0x10 (bank at 0x200000+)

    vdp.write_control_port(0x4000);
    vdp.write_control_port(0x0080);
    let _ = vdp.take_bus_dma_request().expect("request expected");

    // Complete DMA with next_source_addr in a different 128KB region
    vdp.complete_bus_dma(0x0000_1234);
    // LOW/MID updated to reflect new address
    assert_eq!(vdp.register(21), 0x1A); // 0x1234 >> 1 = 0x091A, low = 0x1A
    assert_eq!(vdp.register(22), 0x09); // mid = 0x09
    // HIGH should remain frozen at original value
    assert_eq!(
        vdp.register(23) & 0x7F,
        0x10,
        "source high register must be frozen"
    );
}
