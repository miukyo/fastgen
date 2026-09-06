package com.fastgen.nativebridge;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.SymbolLookup;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

public final class NativeLibLoader {
    private static final String LIB_BASE_NAME = "fastgen_native";

    private NativeLibLoader() {}

    public static SymbolLookup load() {
        String os = System.getProperty("os.name", "").toLowerCase();
        String arch = System.getProperty("os.arch", "").toLowerCase();

        String osFolder;
        String ext;
        String prefix = "lib";

        if (os.contains("win")) {
            osFolder = "windows";
            ext = ".dll";
            prefix = "";
        } else if (os.contains("mac") || os.contains("darwin")) {
            osFolder = "macos";
            ext = ".dylib";
        } else {
            osFolder = "linux";
            ext = ".so";
        }

        String archFolder;
        if (arch.contains("aarch64") || arch.contains("arm64")) {
            archFolder = "aarch64";
        } else if (arch.contains("x86_64") || arch.contains("amd64")) {
            archFolder = "x86_64";
        } else {
            archFolder = arch;
        }

        String libFileName = prefix + LIB_BASE_NAME + ext;
        String platformFolder = osFolder + "-" + archFolder;

        // Check local development paths first
        Path[] devPaths = new Path[] {
                Path.of("crates", "fastgen-native", "target", platformFolder, "release", libFileName),
                Path.of("crates", "fastgen-native", "target", "release", libFileName)
        };
        for (Path devPath : devPaths) {
            if (Files.exists(devPath)) {
                return SymbolLookup.libraryLookup(devPath.toAbsolutePath(), Arena.global());
            }
        }

        // Try candidate embedded resources from jar:
        // 1. /native/linux-x86_64/libfastgen_native.so
        // 2. /native/libfastgen_native.so (flat fallback)
        String[] candidateResources = new String[] {
                "/native/" + platformFolder + "/" + libFileName,
                "/native/" + libFileName
        };

        for (String resourcePath : candidateResources) {
            try (InputStream in = NativeLibLoader.class.getResourceAsStream(resourcePath)) {
                if (in != null) {
                    Path temp = Files.createTempFile("fastgen_" + platformFolder + "_", "_" + libFileName);
                    temp.toFile().deleteOnExit();
                    Files.copy(in, temp, StandardCopyOption.REPLACE_EXISTING);
                    return SymbolLookup.libraryLookup(temp.toAbsolutePath(), Arena.global());
                }
            } catch (IOException e) {
                throw new RuntimeException("Failed extracting native library: " + resourcePath, e);
            }
        }

        // Fallback to system library loader lookup
        return SymbolLookup.loaderLookup();
    }
}
