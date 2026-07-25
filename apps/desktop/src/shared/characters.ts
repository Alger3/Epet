import catSpriteUrl from "../../../../assets/builtin-pet/cat-idle.png";
import humanAvatarSpriteUrl from "../../../../assets/builtin-character/human-avatar.png";
import type { SpriteAtlasDefinition } from "./sprite-atlas";

export type SubjectKind = "pet_cat" | "human_avatar";

export interface CharacterSummary {
  id: string;
  name: string;
  subjectKind: SubjectKind;
  subjectLabel: string;
  description: string;
  assetUrl: string;
  animation?: SpriteAtlasDefinition;
  optionalAction?: string;
}

export const BUILTIN_CHARACTERS: readonly CharacterSummary[] = [
  {
    id: "builtin-orange-tabby",
    name: "橘子",
    subjectKind: "pet_cat",
    subjectLabel: "猫咪",
    description: "温暖的内置橘猫，支持呼吸、点击、拖拽和桌面陪伴。",
    assetUrl: catSpriteUrl,
    animation: {
      imageUrl: catSpriteUrl,
      canvas: { width: 1254, height: 1254 },
      frames: Object.fromEntries(
        ["idle", "walk", "sleep", "tap", "drag", "drop"].map((action) => [
          `${action}_000`,
          {
            frame: { x: 0, y: 0, w: 1254, h: 1254 },
            sourceSize: { w: 1254, h: 1254 },
            spriteSource: { x: 0, y: 0, w: 1254, h: 1254 },
          },
        ]),
      ),
      actions: Object.fromEntries(
        ["idle", "walk", "sleep", "tap", "drag", "drop"].map((action) => [
          action,
          {
            frames: [`${action}_000`],
            frameDurationMs: [100],
            loop: action !== "tap" && action !== "drop",
            fallback: action === "idle" ? null : "idle",
          },
        ]),
      ),
    },
  },
  {
    id: "builtin-forest-guide",
    name: "小栎",
    subjectKind: "human_avatar",
    subjectLabel: "Q 版人物",
    description: "原创成年人物角色，不对应任何真实人物，可完全离线使用。",
    assetUrl: humanAvatarSpriteUrl,
    optionalAction: "挥手",
  },
] as const;

export const DEFAULT_CHARACTER_ID = BUILTIN_CHARACTERS[0].id;

export function findCharacter(characterId: string): CharacterSummary {
  return (
    BUILTIN_CHARACTERS.find((character) => character.id === characterId) ??
    BUILTIN_CHARACTERS[0]
  );
}

export function isSubjectKind(value: string): value is SubjectKind {
  return value === "pet_cat" || value === "human_avatar";
}
