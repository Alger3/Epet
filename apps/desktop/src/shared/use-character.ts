import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import {
  BUILTIN_CHARACTERS,
  findCharacter,
  isSubjectKind,
  type CharacterSummary,
} from "./characters";

const cache = new Map<string, CharacterSummary>();

export function useCharacter(characterId: string): {
  character: CharacterSummary | null;
  error: string | null;
  loading: boolean;
} {
  const builtin = BUILTIN_CHARACTERS.find((character) => character.id === characterId);
  const [character, setCharacter] = useState<CharacterSummary | null>(
    builtin ?? cache.get(characterId) ?? null,
  );
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(!builtin && !cache.has(characterId));
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<string>("character-definition-changed", (event) => {
      cache.delete(event.payload);
      if (!disposed && event.payload === characterId) {
        setRevision((value) => value + 1);
      }
    }).then((dispose) => {
      if (disposed) {
        dispose();
      } else {
        unlisten = dispose;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [characterId]);

  useEffect(() => {
    const selectedBuiltin = BUILTIN_CHARACTERS.find((item) => item.id === characterId);
    if (selectedBuiltin) {
      setCharacter(selectedBuiltin);
      setError(null);
      setLoading(false);
      return;
    }
    const cached = revision === 0 ? cache.get(characterId) : undefined;
    if (cached) {
      setCharacter(cached);
      setError(null);
      setLoading(false);
      return;
    }
    if (!("__TAURI_INTERNALS__" in window)) {
      setCharacter(findCharacter(characterId));
      setError("浏览器预览无法读取本地安装角色");
      setLoading(false);
      return;
    }

    let disposed = false;
    setCharacter(null);
    setError(null);
    setLoading(true);
    void invoke<CharacterSummary>("get_character_definition", { characterId })
      .then((definition) => {
        if (disposed) return;
        if (!isSubjectKind(definition.subjectKind) || definition.id !== characterId) {
          throw new Error("角色运行时定义与当前角色不一致");
        }
        cache.set(characterId, definition);
        setCharacter(definition);
      })
      .catch((reason) => {
        if (!disposed) setError(reason instanceof Error ? reason.message : String(reason));
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });

    return () => {
      disposed = true;
    };
  }, [characterId, revision]);

  return { character, error, loading };
}
