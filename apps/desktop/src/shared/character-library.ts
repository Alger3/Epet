import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

export interface CharacterVersion {
  packageVersion: string;
  packageSha256: string;
  packageSize: number;
  installedAt: string;
  sourceUrl: string | null;
  current: boolean;
  localAvailable: boolean;
}

export interface CharacterLibraryItem {
  id: string;
  name: string;
  subjectKind: "pet_cat" | "human_avatar";
  builtIn: boolean;
  currentPackageSha256: string | null;
  currentVersion: string | null;
  createdAt: string;
  updatedAt: string;
  versions: CharacterVersion[];
  localAvailable: boolean;
}

const isTauriRuntime = (): boolean => "__TAURI_INTERNALS__" in window;

export function useCharacterLibrary() {
  const [characters, setCharacters] = useState<CharacterLibraryItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    if (!isTauriRuntime()) return;
    setError(null);
    try {
      setCharacters(await invoke<CharacterLibraryItem[]>("list_character_library"));
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = useCallback(
    async <T,>(operation: () => Promise<T>) => {
      setBusy(true);
      setError(null);
      try {
        const result = await operation();
        await reload();
        return result;
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
        return undefined;
      } finally {
        setBusy(false);
      }
    },
    [reload],
  );

  return {
    characters,
    busy,
    error,
    reload,
    installFromUrl: (url: string, expectedSha256: string) =>
      run(() =>
        invoke<CharacterLibraryItem>("install_pet_package_from_url", {
          url,
          expectedSha256,
        }),
      ),
    activateVersion: (characterId: string, packageSha256: string) =>
      run(() =>
        invoke<CharacterLibraryItem>("activate_character_version", {
          characterId,
          packageSha256,
        }),
      ),
    deleteVersion: (characterId: string, packageSha256: string) =>
      run(() =>
        invoke<CharacterLibraryItem>("delete_character_version", {
          characterId,
          packageSha256,
        }),
      ),
    deleteCharacter: (characterId: string) =>
      run(() => invoke<void>("delete_installed_character", { characterId })),
    renameCharacter: (characterId: string, customName: string) =>
      run(() =>
        invoke<void>("rename_installed_character", { characterId, customName }),
      ),
  };
}
