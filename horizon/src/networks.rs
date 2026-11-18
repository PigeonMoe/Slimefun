// SPDX-License-Identifier: GPL-3.0-only

/// The block identities in Paper's SlimefunItemSetup, EnergyNet and CargoNet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Component {
    EnergyRegulator,
    GpsTransmitter {
        capacity: u32,
    },
    GeoScanner,
    ManualCrucible,
    IgnitionChamber,
    EnergyConnector,
    Capacitor(u32),
    ElectricMachine {
        capacity: u32,
        carrier: &'static str,
    },
    FuelGenerator {
        capacity: u32,
        carrier: &'static str,
    },
    SolarGenerator {
        day: u32,
        night: u32,
    },
    CargoManager,
    CargoConnector,
    TrashCan,
    ReactorPort,
    WitherProofObsidian,
    DuctTape,
    CargoInput,
    CargoOutput {
        advanced: bool,
    },
}

impl Component {
    pub(crate) fn for_item(id: &str) -> Option<Self> {
        if let Some(transmitter) = crate::geology::transmitter(id) {
            return Some(Self::GpsTransmitter {
                capacity: transmitter.capacity,
            });
        }
        if let Some(generator) = crate::fuel_generators::generator(id) {
            return Some(Self::FuelGenerator {
                capacity: generator.capacity,
                carrier: generator.item.material(),
            });
        }
        if let Some(reactor) = crate::factory::reactors::reactor(id) {
            return Some(Self::FuelGenerator {
                capacity: reactor.capacity,
                carrier: reactor.item.material(),
            });
        }
        Some(match id {
            "REACTOR_ACCESS_PORT" => Self::ReactorPort,
            "WITHER_PROOF_OBSIDIAN" => Self::WitherProofObsidian,
            "DUCT_TAPE" => Self::DuctTape,
            "TRASH_CAN_BLOCK" => Self::TrashCan,
            "GPS_GEO_SCANNER" => Self::GeoScanner,
            "CRUCIBLE" => Self::ManualCrucible,
            "IGNITION_CHAMBER" => Self::IgnitionChamber,
            "OIL_PUMP" => Self::ElectricMachine {
                capacity: crate::geology::PUMP_CAPACITY,
                carrier: "minecraft:player_head",
            },
            "FLUID_PUMP" => Self::ElectricMachine {
                capacity: crate::factory::fluid_pump::CAPACITY,
                carrier: crate::factory::fluid_pump::FLUID_PUMP.material(),
            },
            "XP_COLLECTOR" => Self::ElectricMachine {
                capacity: crate::factory::experience_collector::CAPACITY,
                carrier: crate::factory::experience_collector::XP_COLLECTOR.material(),
            },
            "ENERGY_REGULATOR" => Self::EnergyRegulator,
            "ENERGY_CONNECTOR" => Self::EnergyConnector,
            "SMALL_CAPACITOR" => Self::Capacitor(128),
            "MEDIUM_CAPACITOR" => Self::Capacitor(512),
            "BIG_CAPACITOR" => Self::Capacitor(1_024),
            "LARGE_CAPACITOR" => Self::Capacitor(8_192),
            "CARBONADO_EDGED_CAPACITOR" => Self::Capacitor(65_536),
            "ENERGIZED_CAPACITOR" => Self::Capacitor(524_288),
            "ELECTRIC_FURNACE" => Self::ElectricMachine {
                capacity: 64,
                carrier: "minecraft:furnace",
            },
            "ELECTRIC_FURNACE_2" | "ELECTRIC_FURNACE_3" | "ELECTRIC_ORE_GRINDER" => {
                Self::ElectricMachine {
                    capacity: 128,
                    carrier: "minecraft:furnace",
                }
            }
            "ELECTRIC_ORE_GRINDER_2" | "ELECTRIC_INGOT_PULVERIZER" => Self::ElectricMachine {
                capacity: 512,
                carrier: "minecraft:furnace",
            },
            "ELECTRIC_ORE_GRINDER_3" => Self::ElectricMachine {
                capacity: 1_024,
                carrier: "minecraft:furnace",
            },
            "HEATED_PRESSURE_CHAMBER" => Self::ElectricMachine {
                capacity: 128,
                carrier: "minecraft:light_gray_stained_glass",
            },
            "HEATED_PRESSURE_CHAMBER_2" => Self::ElectricMachine {
                capacity: 256,
                carrier: "minecraft:light_gray_stained_glass",
            },
            "SOLAR_GENERATOR" => Self::SolarGenerator { day: 2, night: 0 },
            "SOLAR_GENERATOR_2" => Self::SolarGenerator { day: 8, night: 0 },
            "SOLAR_GENERATOR_3" => Self::SolarGenerator { day: 32, night: 0 },
            "SOLAR_GENERATOR_4" => Self::SolarGenerator {
                day: 128,
                night: 64,
            },
            "CARGO_MANAGER" => Self::CargoManager,
            "CARGO_NODE" => Self::CargoConnector,
            "CARGO_NODE_INPUT" => Self::CargoInput,
            "CARGO_NODE_OUTPUT" => Self::CargoOutput { advanced: false },
            "CARGO_NODE_OUTPUT_ADVANCED" => Self::CargoOutput { advanced: true },
            _ => {
                if let Some(machine) = crate::electric_processing::machine(id) {
                    Self::ElectricMachine {
                        capacity: machine.capacity,
                        carrier: machine.item.material(),
                    }
                } else {
                    let machine = crate::advanced_processing::machine(id)?;
                    Self::ElectricMachine {
                        capacity: machine.capacity,
                        carrier: machine.item.material(),
                    }
                }
            }
        })
    }

    pub(crate) fn describe(self, metadata: &toml::Table) -> String {
        match self {
            Self::GpsTransmitter { capacity } => {
                format!("GPS transmitter | Paper buffer {capacity} J")
            }
            Self::ManualCrucible => "Manual Crucible".into(),
            Self::IgnitionChamber => "Automatic Ignition Chamber".into(),
            Self::GeoScanner => "GPS Geo-Scanner | Requires 600 GPS complexity".into(),
            Self::EnergyRegulator => "Energy regulator | Paper connection range 6".into(),
            Self::EnergyConnector => "Energy connector | Paper connection range 6".into(),
            Self::Capacitor(capacity) => {
                let charge = metadata.get("energy-charge").and_then(toml::Value::as_str);
                match charge {
                    None => format!("Capacitor | Stored charge 0 / {capacity} J"),
                    Some(field) => match field.parse::<u32>() {
                        Ok(value) if value <= capacity => {
                            format!("Capacitor | Stored charge {value} / {capacity} J")
                        }
                        _ => format!("Capacitor | Stored charge is invalid for {capacity} J"),
                    },
                }
            }
            Self::SolarGenerator { day, night } => {
                format!("Solar generator | Paper generation {day} J day / {night} J night")
            }
            Self::ElectricMachine { capacity, .. } => {
                format!("Electric machine | Paper buffer {capacity} J")
            }
            Self::FuelGenerator { capacity, .. } => {
                format!("Fuel generator | Paper buffer {capacity} J")
            }
            Self::CargoManager => "Cargo manager | Paper connection range 5".into(),
            Self::CargoConnector => "Cargo connector | Paper connection range 5".into(),
            Self::TrashCan => "Trash can | Seven input slots".into(),
            Self::ReactorPort => "Reactor Access Port | Three blocks above a Reactor".into(),
            Self::WitherProofObsidian => "Wither-Proof Obsidian".into(),
            Self::DuctTape => "Duct Tape".into(),
            Self::CargoInput => cargo_terminal("Cargo input", metadata),
            Self::CargoOutput { advanced: false } => cargo_terminal("Cargo output", metadata),
            Self::CargoOutput { advanced: true } => {
                cargo_terminal("Advanced cargo output", metadata)
            }
        }
    }
}

fn cargo_terminal(role: &str, metadata: &toml::Table) -> String {
    let frequency = metadata.get("frequency").and_then(toml::Value::as_str);
    match frequency.map(str::parse::<u8>) {
        None => format!("{role} | Channel 1"),
        Some(Ok(channel @ 0..=15)) => format!("{role} | Channel {}", channel + 1),
        Some(Ok(16)) => format!("{role} | Terminal channel 17"),
        _ => format!("{role} | Invalid stored channel"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_saved_paper_capacitor_and_cargo_ids() {
        let capacitor = Component::for_item("SMALL_CAPACITOR").unwrap();
        let cargo = Component::for_item("CARGO_NODE_OUTPUT_ADVANCED").unwrap();
        let metadata = "energy-charge = \"42\"\nfrequency = \"15\""
            .parse::<toml::Table>()
            .unwrap();
        assert_eq!(
            capacitor.describe(&metadata),
            "Capacitor | Stored charge 42 / 128 J"
        );
        assert_eq!(
            cargo.describe(&metadata),
            "Advanced cargo output | Channel 16"
        );
        assert_eq!(
            Component::for_item("SOLAR_GENERATOR_4"),
            Some(Component::SolarGenerator {
                day: 128,
                night: 64
            })
        );
        assert_eq!(Component::for_item("UNREGISTERED_MACHINE"), None);
    }
}
