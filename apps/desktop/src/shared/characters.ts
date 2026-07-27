import catSpriteUrl from "../../../../assets/builtin-pet/cat-idle.png";
import catAnimationUrl from "../../../../assets/builtin-pet/animation-atlas.png";
import catAnimationData from "../../../../assets/builtin-pet/animation.json";
import humanAvatarSpriteUrl from "../../../../assets/builtin-character/human-avatar.png";
import humanAnimationUrl from "../../../../assets/builtin-character/animation-atlas.png";
import humanAnimationData from "../../../../assets/builtin-character/animation.json";
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

function builtinAnimation(
  imageUrl: string,
  data: Omit<SpriteAtlasDefinition, "imageUrl">,
): SpriteAtlasDefinition {
  return { ...data, imageUrl };
}

export const BUILTIN_CHARACTERS: readonly CharacterSummary[] = [
  {
    id: "builtin-orange-tabby",
    name: "橘子",
    subjectKind: "pet_cat",
    subjectLabel: "猫咪",
    description: "内置动画测试猫，支持眨眼、呼吸、四肢走动、摇尾巴和卧姿睡眠。",
    assetUrl: catSpriteUrl,
    animation: builtinAnimation(
      catAnimationUrl,
      catAnimationData as Omit<SpriteAtlasDefinition, "imageUrl">,
    ),
  },
  {
    id: "builtin-forest-guide",
    name: "小栎",
    subjectKind: "human_avatar",
    subjectLabel: "Q 版人物",
    description: "原创内置动画测试人物，支持呼吸、走路、闭眼睡眠和头发次级运动。",
    assetUrl: humanAvatarSpriteUrl,
    animation: builtinAnimation(
      humanAnimationUrl,
      humanAnimationData as Omit<SpriteAtlasDefinition, "imageUrl">,
    ),
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
