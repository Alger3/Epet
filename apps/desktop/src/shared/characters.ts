import catSpriteUrl from "../../../../assets/builtin-pet/cat-idle.png";
import humanAvatarSpriteUrl from "../../../../assets/builtin-character/human-avatar.png";

export type SubjectKind = "pet_cat" | "human_avatar";

export interface CharacterSummary {
  id: string;
  name: string;
  subjectKind: SubjectKind;
  subjectLabel: string;
  description: string;
  assetUrl: string;
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
