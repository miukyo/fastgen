package com.fastgen.generator;

/**
 * Supported Minecraft world generation target versions.
 */
public enum WorldgenVersion {
    V26_1("26.1", 1),
    V26_2("26.2", 2);

    private final String label;
    private final int nativeId;

    WorldgenVersion(String label, int nativeId) {
        this.label = label;
        this.nativeId = nativeId;
    }

    public String getLabel() {
        return label;
    }

    public int getNativeId() {
        return nativeId;
    }

    public static WorldgenVersion parse(String str) {
        if (str == null) return V26_2;
        String s = str.trim().toLowerCase();
        if (s.contains("26.1") || s.equals("1") || s.contains("1.21")) {
            return V26_1;
        }
        return V26_2;
    }

    @Override
    public String toString() {
        return label;
    }
}
