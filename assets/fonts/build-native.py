from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont


directory = Path(__file__).resolve().parent
family = "Metra Inter"
license_text = (directory / "OFL-Inter.txt").read_text(encoding="utf-8").strip()
weights = {
    400: "Regular",
    500: "Medium",
    550: "Book",
    650: "DemiBold",
    700: "Bold",
    750: "Heavy",
}

for weight, style in weights.items():
    with TTFont(directory / "InterVariable.ttf", recalcTimestamp=False) as source:
        face = instantiateVariableFont(source, {"opsz": 14, "wght": weight}, static=True)
        names = face["name"]
        full_name = f"{family} {style}"
        postscript_name = f"MetraInter-{style}"
        for name_id, name in {
            1: family,
            2: style,
            3: f"{names.getDebugName(5)};{postscript_name}",
            4: full_name,
            6: postscript_name,
            13: license_text,
            16: family,
            17: style,
            21: family,
            22: style,
            25: None,
        }.items():
            names.removeNames(nameID=name_id)
            if name is not None:
                names.setName(name, name_id, 3, 1, 0x409)
                names.setName(name, name_id, 1, 0, 0)
        selection = face["OS/2"]
        selection.fsSelection &= ~(0x20 | 0x40)
        if weight == 400:
            selection.fsSelection |= 0x40
        if weight == 700:
            selection.fsSelection |= 0x20
            face["head"].macStyle |= 1
        face.save(directory / f"MetraInter-{weight}.ttf")
