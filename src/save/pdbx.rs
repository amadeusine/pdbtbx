use crate::error::*;
use crate::structs::*;
use crate::validate;
use crate::StrictnessLevel;

use std::fs::File;
use std::io::prelude::*;
use std::io::BufWriter;

/// Save the given PDB struct to the given file as mmCIF or PDBx.
/// It validates the PDB. It fails if the validation fails with the given `level`.
/// If validation gives rise to problems use the `save_raw` function.
/// ## Warning
/// This function is unstable and unfinished!
pub fn save_mmcif(
    pdb: PDB,
    filename: &str,
    level: StrictnessLevel,
    name: &str,
) -> Result<(), Vec<PDBError>> {
    let mut errors = validate(&pdb);
    for error in &errors {
        if error.fails(level) {
            return Err(errors);
        }
    }

    let file = match File::create(filename) {
        Ok(f) => f,
        Err(_e) => {
            errors.push(PDBError::new(
                ErrorLevel::BreakingError,
                "Could not open file",
                "Could not open the file for writing, make sure you have permission for this file and no other program is currently using it.",
                Context::show(filename)
            ));
            return Err(errors);
        }
    };

    save_mmcif_raw(&pdb, BufWriter::new(file), name);
    Ok(())
}

/// Save the given PDB struct to the given BufWriter.
/// It does not validate or renumber the PDB, so if that is needed that needs to be done in preparation.
/// It does change the output format based on the StrictnessLevel given.
///
/// ## Warning
/// This function is unstable and unfinished!
#[allow(clippy::unwrap_used)]
pub fn save_mmcif_raw<T: Write>(pdb: &PDB, mut sink: BufWriter<T>, name: &str) {
    macro_rules! write {
        ($($arg:tt)*) => {
            sink.write_fmt(format_args!($($arg)*)).unwrap();
            sink.write_all(b"\n").unwrap();
        }
    }

    // Header
    write!(
        "data_{}
# 
_entry.id   {} 
# 
_audit_conform.dict_name       mmcif_pdbx.dic 
_audit_conform.dict_version    5.279 
_audit_conform.dict_location   http://mmcif.pdb.org/dictionaries/ascii/mmcif_pdbx.dic",
        name,
        name
    );

    // Cryst
    if pdb.has_unit_cell() {
        let unit_cell = pdb.unit_cell();
        write!(
            "# 
_cell.entry_id           {}
_cell.length_a           {} 
_cell.length_b           {} 
_cell.length_c           {} 
_cell.angle_alpha        {}
_cell.angle_beta         {}
_cell.angle_gamma        {}
_cell.Z_PDB              {} 
_cell.pdbx_unique_axis   ?
_cell.length_a_esd       ?
_cell.length_b_esd       ?
_cell.length_c_esd       ?
_cell.angle_alpha_esd    ?
_cell.angle_beta_esd     ?
_cell.angle_gamma_esd    ?",
            name,
            unit_cell.a(),
            unit_cell.b(),
            unit_cell.c(),
            unit_cell.alpha(),
            unit_cell.beta(),
            unit_cell.gamma(),
            if pdb.has_symmetry() {
                pdb.symmetry().z().to_string()
            } else {
                "?".to_owned()
            }
        );
    }

    if pdb.has_symmetry() {
        write!(
            "# 
_symmetry.entry_id                         {} 
_symmetry.space_group_name_H-M             '{}' 
_symmetry.pdbx_full_space_group_name_H-M   ? 
_symmetry.cell_setting                     ? 
_symmetry.Int_Tables_number                {} 
_symmetry.space_group_name_Hall            ? ",
            name,
            pdb.symmetry().symbol(),
            pdb.symmetry().index()
        );
    }

    // Models
    write!(
        "# 
loop_
_atom_site.group_PDB 
_atom_site.id 
_atom_site.type_symbol 
_atom_site.label_atom_id 
_atom_site.label_alt_id 
_atom_site.label_comp_id 
_atom_site.label_asym_id 
_atom_site.label_entity_id 
_atom_site.label_seq_id 
_atom_site.pdbx_PDB_ins_code 
_atom_site.Cartn_x 
_atom_site.Cartn_y 
_atom_site.Cartn_z 
_atom_site.occupancy 
_atom_site.B_iso_or_equiv 
_atom_site.pdbx_formal_charge 
_atom_site.pdbx_PDB_model_num"
    );
    let mut lines = Vec::new();
    for model in pdb.models() {
        for (index, chain) in model.chains().enumerate() {
            for residue in chain.residues() {
                for atom in residue.atoms() {
                    lines.push([
                        "ATOM".to_string(),
                        atom.serial_number().to_string(),
                        atom.element().to_string(),
                        atom.name().to_string(),
                        ".".to_string(),
                        residue.id().to_string(),
                        chain.id().to_string(),
                        (index + 1).to_string(),
                        residue.serial_number().to_string(),
                        "?".to_string(),
                        atom.x().to_string(),
                        atom.y().to_string(),
                        atom.z().to_string(),
                        atom.occupancy().to_string(),
                        atom.b_factor().to_string(),
                        if atom.charge() == 0 {
                            "?".to_string()
                        } else {
                            atom.charge().to_string()
                        },
                        model.serial_number().to_string(),
                    ]);
                }
            }
        }
        for (index, chain) in model.hetero_chains().enumerate() {
            for residue in chain.residues() {
                for atom in residue.atoms() {
                    lines.push([
                        "HETATM".to_string(),
                        atom.serial_number().to_string(),
                        atom.element().to_string(),
                        atom.name().to_string(),
                        ".".to_string(),
                        residue.id().to_string(),
                        chain.id().to_string(),
                        (index + 1 + model.chain_count()).to_string(),
                        residue.serial_number().to_string(),
                        "?".to_string(),
                        atom.x().to_string(),
                        atom.y().to_string(),
                        atom.z().to_string(),
                        atom.occupancy().to_string(),
                        atom.b_factor().to_string(),
                        if atom.charge() == 0 {
                            "?".to_string()
                        } else {
                            atom.charge().to_string()
                        },
                        model.serial_number().to_string(),
                    ]);
                }
            }
        }
    }
    // Now align the table
    let mut sizes = [1; 19];
    for line in &lines {
        for index in 0..line.len() {
            sizes[index] = std::cmp::max(sizes[index], line[index].len());
        }
    }
    // Now write the table
    for line in lines {
        let mut output = String::new();
        output.push_str(&line[0]);
        output.push_str(&" ".repeat(sizes[0] - line[0].len()));
        for index in 1..line.len() {
            output.push(' ');
            if line[index].trim() != "" {
                output.push_str(&line[index]);
                output.push_str(&" ".repeat(sizes[index] - line[index].len()));
            } else {
                output.push('?');
                output.push_str(&" ".repeat(sizes[index] - 1));
            }
        }
        output.push('\n');
        sink.write_all(output.as_bytes()).unwrap();
    }

    write!("#");

    sink.flush().unwrap();
}