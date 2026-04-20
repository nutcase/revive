use super::spec::MapperSpec;
use crate::cartridge::{
    BandaiFcg, Fme7, IremG101, IremH3001, JalecoSs88006, Mapper15, Mapper246, Mapper40, Mapper42,
    Mapper43, Mapper50, Mmc1, Mmc2, Mmc3, Mmc5, Namco163, Namco210, Sunsoft3, Sunsoft4,
    TaitoTc0190, TaitoX1005, TaitoX1017, Vrc1, Vrc2Vrc4, Vrc3, Vrc6,
};

pub(super) struct MapperInstances {
    pub(super) mmc1: Option<Mmc1>,
    pub(super) mmc2: Option<Mmc2>,
    pub(super) mmc3: Option<Mmc3>,
    pub(super) mmc5: Option<Mmc5>,
    pub(super) namco163: Option<Namco163>,
    pub(super) namco210: Option<Namco210>,
    pub(super) jaleco_ss88006: Option<JalecoSs88006>,
    pub(super) vrc2_vrc4: Option<Vrc2Vrc4>,
    pub(super) mapper40: Option<Mapper40>,
    pub(super) mapper42: Option<Mapper42>,
    pub(super) mapper43: Option<Mapper43>,
    pub(super) mapper50: Option<Mapper50>,
    pub(super) fme7: Option<Fme7>,
    pub(super) bandai_fcg: Option<BandaiFcg>,
    pub(super) irem_g101: Option<IremG101>,
    pub(super) irem_h3001: Option<IremH3001>,
    pub(super) vrc1: Option<Vrc1>,
    pub(super) vrc3: Option<Vrc3>,
    pub(super) vrc6: Option<Vrc6>,
    pub(super) mapper15: Option<Mapper15>,
    pub(super) sunsoft3: Option<Sunsoft3>,
    pub(super) sunsoft4: Option<Sunsoft4>,
    pub(super) taito_tc0190: Option<TaitoTc0190>,
    pub(super) taito_x1005: Option<TaitoX1005>,
    pub(super) taito_x1017: Option<TaitoX1017>,
    pub(super) mapper246: Option<Mapper246>,
}

impl MapperInstances {
    pub(super) fn new(spec: MapperSpec) -> Self {
        let mut vrc2_vrc4 = spec.uses_vrc2_vrc4().then(Vrc2Vrc4::new);
        if spec.vrc2_vrc4_starts_in_vrc4_mode() {
            if let Some(vrc) = vrc2_vrc4.as_mut() {
                vrc.vrc4_mode = true;
            }
        }

        Self {
            mmc1: spec.uses_mmc1().then(Mmc1::new),
            mmc2: spec.uses_mmc2().then(Mmc2::new),
            mmc3: spec.uses_mmc3().then(Mmc3::new),
            mmc5: spec.uses_mmc5().then(Mmc5::new),
            namco163: spec.uses_namco163().then(Namco163::new),
            namco210: spec
                .uses_namco210()
                .then(|| Namco210::new(spec.namco210_hardwired_mirroring())),
            jaleco_ss88006: spec.uses_jaleco_ss88006().then(JalecoSs88006::new),
            vrc2_vrc4,
            mapper40: spec.uses_mapper40().then(Mapper40::new),
            mapper42: spec.uses_mapper42().then(Mapper42::new),
            mapper43: spec.uses_mapper43().then(Mapper43::new),
            mapper50: spec.uses_mapper50().then(Mapper50::new),
            fme7: spec.uses_fme7().then(Fme7::new),
            bandai_fcg: spec.uses_bandai_fcg().then(BandaiFcg::new),
            irem_g101: spec.uses_irem_g101().then(IremG101::new),
            irem_h3001: spec.uses_irem_h3001().then(IremH3001::new),
            vrc1: spec.uses_vrc1().then(Vrc1::new),
            vrc3: spec.uses_vrc3().then(Vrc3::new),
            vrc6: spec.uses_vrc6().then(Vrc6::new),
            mapper15: spec.uses_mapper15().then(Mapper15::new),
            sunsoft3: spec.uses_sunsoft3().then(Sunsoft3::new),
            sunsoft4: spec.uses_sunsoft4().then(Sunsoft4::new),
            taito_tc0190: spec.uses_taito_tc0190().then(TaitoTc0190::new),
            taito_x1005: spec.uses_taito_x1005().then(TaitoX1005::new),
            taito_x1017: spec.uses_taito_x1017().then(TaitoX1017::new),
            mapper246: spec.uses_mapper246().then(Mapper246::new),
        }
    }
}
