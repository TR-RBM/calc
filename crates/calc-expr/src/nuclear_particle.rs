const NUCLIDE_CODE: i64 = 0;
const ELECTRON_CODE: i64 = 1;
const POSITRON_CODE: i64 = 2;
const ELECTRON_NEUTRINO_CODE: i64 = 3;
const ELECTRON_ANTINEUTRINO_CODE: i64 = 4;
const PHOTON_CODE: i64 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NuclearParticle {
    Nuclide {
        mass_number: u32,
        atomic_number: u32,
    },
    Electron,
    Positron,
    ElectronNeutrino,
    ElectronAntineutrino,
    Photon,
}

impl NuclearParticle {
    pub fn codes(self) -> [i64; 3] {
        match self {
            NuclearParticle::Nuclide {
                mass_number,
                atomic_number,
            } => [
                NUCLIDE_CODE,
                i64::from(mass_number),
                i64::from(atomic_number),
            ],
            NuclearParticle::Electron => [ELECTRON_CODE, 0, 0],
            NuclearParticle::Positron => [POSITRON_CODE, 0, 0],
            NuclearParticle::ElectronNeutrino => [ELECTRON_NEUTRINO_CODE, 0, 0],
            NuclearParticle::ElectronAntineutrino => [ELECTRON_ANTINEUTRINO_CODE, 0, 0],
            NuclearParticle::Photon => [PHOTON_CODE, 0, 0],
        }
    }

    pub fn from_codes(codes: &[i64]) -> Option<Self> {
        let [kind, mass_number, atomic_number] = codes else {
            return None;
        };
        match *kind {
            NUCLIDE_CODE => Some(NuclearParticle::Nuclide {
                mass_number: u32::try_from(*mass_number).ok()?,
                atomic_number: u32::try_from(*atomic_number).ok()?,
            }),
            ELECTRON_CODE => Some(NuclearParticle::Electron),
            POSITRON_CODE => Some(NuclearParticle::Positron),
            ELECTRON_NEUTRINO_CODE => Some(NuclearParticle::ElectronNeutrino),
            ELECTRON_ANTINEUTRINO_CODE => Some(NuclearParticle::ElectronAntineutrino),
            PHOTON_CODE => Some(NuclearParticle::Photon),
            _ => None,
        }
    }

    pub fn nucleons(self) -> i64 {
        match self {
            NuclearParticle::Nuclide { mass_number, .. } => i64::from(mass_number),
            _ => 0,
        }
    }

    pub fn charge(self) -> i64 {
        match self {
            NuclearParticle::Nuclide { atomic_number, .. } => i64::from(atomic_number),
            NuclearParticle::Electron => -1,
            NuclearParticle::Positron => 1,
            _ => 0,
        }
    }

    pub fn electron_leptons(self) -> i64 {
        match self {
            NuclearParticle::Electron | NuclearParticle::ElectronNeutrino => 1,
            NuclearParticle::Positron | NuclearParticle::ElectronAntineutrino => -1,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_particle_survives_its_codes() {
        let all = [
            NuclearParticle::Nuclide {
                mass_number: 14,
                atomic_number: 6,
            },
            NuclearParticle::Electron,
            NuclearParticle::Positron,
            NuclearParticle::ElectronNeutrino,
            NuclearParticle::ElectronAntineutrino,
            NuclearParticle::Photon,
        ];
        for particle in all {
            assert_eq!(
                NuclearParticle::from_codes(&particle.codes()),
                Some(particle)
            );
        }
    }

    #[test]
    fn a_positron_carries_charge_one_and_lepton_number_minus_one() {
        assert_eq!(NuclearParticle::Positron.charge(), 1);
        assert_eq!(NuclearParticle::Positron.electron_leptons(), -1);
        assert_eq!(NuclearParticle::Positron.nucleons(), 0);
    }
}
