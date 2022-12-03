octave1 = {
    C = 32.70,
    Cs = 34.65,
    D = 36.71,
    Ds = 38.89,
    E = 41.20,
    F = 43.65,
    Fs = 46.25,
    G = 49.00,
    Gs = 51.91,
    A = 55.00,
    As = 58.27,
    B = 61.74
}
octave2 = {
    C = 65.41,
    Cs = 69.30,
    D = 73.42,
    Ds = 77.78,
    E = 82.41,
    F = 87.31,
    Fs = 92.50,
    G = 98.00,
    Gs = 103.83,
    A = 110.00,
    As = 116.54,
    Bb = 116.54,
    B = 123.47

}

octave4 = {
    C = 261.63,
    Cs = 277.18,
    D = 293.66,
    Ds = 311.13,
    E = 329.63,
    F = 349.23,
    Fs = 369.99,
    G = 392.00,
    Gs = 415.30,
    A = 440.00,
    As = 466.16,
    Bb = 466.16,
    B = 493.88
}
octave5 = {
    C = 523.25,
    Cs = 554.37,
    D = 587.33,
    Ds = 622.25,
    E = 659.26,
    F = 698.46,
    Fs = 739.99,
    G = 783.99,
    Gs = 830.61,
    A = 880.00,
    As = 932.33,
    Bb = 932.33,
    B = 987.77
}
octave6 = {
    C = 1046.50,
    Cs = 1108.73,
    D = 1174.66,
    Ds = 1244.51,
    E = 1318.51,
    F = 1396.91,
    Fs = 1479.98,
    G = 1567.98,
    Gs = 1661.22,
    A = 1760.00,
    As = 1864.66,
    Bb = 1864.66,
    B = 1975.53
}

-- ocatave 3
notes = {
    B1 = octave1["B"],
    B2 = octave2["B"],
    C2 = octave2["C"],
    C = 130.81,
    Cs = 138.59,
    D = 146.83,
    Ds = 155.56,
    E = 164.81,
    F = 174.61,
    Fs = 185.00,
    G = 196.00,
    Gs = 207.65,
    A = 220.00,
    As = 233.08,
    B = 246.94,
    Z = -1,
    _ = 0,
    C4 = octave4["C"],
    Cs4 = octave4["Cs"],
    D4 = octave4["D"],
    Ds4 = octave4["Ds"],
    E4 = octave4["E"],
    F4 = octave4["F"],
    Fs4 = octave4["Fs"],
    G4 = octave4["G"],
    A4 = octave4["A"],
    Bb4 = octave4["Bb"],
    B4 = octave4["B"],
    C5 = octave5["C"],
    Cs5 = octave5["Cs"],
    D5 = octave5["D"],
    Ds5 = octave5["Ds"],
    E5 = octave5["E"],
    F5 = octave5["F"],
    Fs5 = octave5["Fs"],
    G5 = octave5["G"],
    A5 = octave5["A"],
    Bb5 = octave5["Bb"],
    B5 = octave5["B"],
    C6 = octave6["C"],
    Cs6 = octave6["Cs"],
    D6 = octave6["D"],
    Ds6 = octave6["Ds"],
    E6 = octave6["E"],
    F6 = octave6["F"]
}
-- no valve
-- brass = {"C2", "C", "G", "C4", "E4", "G4", "Bb4", "D5"}
-- 2nd valve
-- brass = {"B1", "B2", "Fs", "B", "Ds4", "Fs4", "A4", "C5"}
-- test???
-- brass = {"C4", "D4", "E4", "F4", "G4", "A4", "B4"}
brass = { "C", "C4", "G4", "C5", "E5", "G5", "Bb5", "C6", "D6", "E6" }

-- song = {"G", "E", "G", "G", "E", "G", "A", "G", "F", "E", "D", "E", "F"}

-- song = {"E", "C", "C", "E", "F", "F", "F", "F", "G", "G", "G", "A", "A", "G", "E", "C", "Z"}
-- song = {"E", "A", "_", "E", "F", "_", "F", "G", "_", "A", "G", "E", "D", "_",}
-- song = {"E", "A", "_", "E", "F", "_", "F", "G", "_", "A", "G", "E", "D", "_"}

-- song = {"A", "C", "_", "A", ""}
