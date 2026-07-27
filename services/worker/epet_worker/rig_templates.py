"""Versioned, deterministic rig templates used by the offline Mock pipeline."""

from copy import deepcopy


def _bone(
    bone_id: str,
    parent: str | None,
    origin: tuple[float, float],
    length: float,
    angle: float = 0,
    limit: tuple[float, float] = (-45, 45),
) -> dict:
    return {
        "id": bone_id,
        "parent": parent,
        "origin": list(origin),
        "length": length,
        "angle": angle,
        "angle_limit": list(limit),
    }


CAT_BONES = [
    _bone("root", None, (0.5, 0.82), 0, limit=(0, 0)),
    _bone("torso", "root", (0.5, 0.64), 0.3, -90, (-110, -70)),
    _bone("head", "torso", (0.5, 0.38), 0.16, -90, (-125, -55)),
    _bone("ear_l", "head", (0.42, 0.23), 0.09, -115, (-145, -80)),
    _bone("ear_r", "head", (0.58, 0.23), 0.09, -65, (-100, -35)),
    _bone("front_leg_l", "torso", (0.41, 0.66), 0.22, 90, (45, 135)),
    _bone("front_paw_l", "front_leg_l", (0.39, 0.86), 0.08, 0, (-35, 35)),
    _bone("front_leg_r", "torso", (0.57, 0.66), 0.22, 90, (45, 135)),
    _bone("front_paw_r", "front_leg_r", (0.59, 0.86), 0.08, 0, (-35, 35)),
    _bone("hind_leg_l", "torso", (0.32, 0.67), 0.2, 90, (45, 140)),
    _bone("hind_paw_l", "hind_leg_l", (0.29, 0.86), 0.1, 0, (-35, 35)),
    _bone("hind_leg_r", "torso", (0.68, 0.67), 0.2, 90, (40, 135)),
    _bone("hind_paw_r", "hind_leg_r", (0.71, 0.86), 0.1, 0, (-35, 35)),
    _bone("tail_01", "torso", (0.72, 0.64), 0.16, -10, (-70, 55)),
    _bone("tail_02", "tail_01", (0.84, 0.61), 0.14, -30, (-85, 70)),
    _bone("tail_03", "tail_02", (0.91, 0.52), 0.12, -65, (-100, 90)),
    _bone("tail_04", "tail_03", (0.92, 0.42), 0.1, -95, (-125, 100)),
]

HUMAN_BONES = [
    _bone("root", None, (0.5, 0.91), 0, limit=(0, 0)),
    _bone("pelvis", "root", (0.5, 0.64), 0.1, -90, (-105, -75)),
    _bone("torso", "pelvis", (0.5, 0.48), 0.2, -90, (-110, -70)),
    _bone("head", "torso", (0.5, 0.28), 0.16, -90, (-120, -60)),
    _bone("upper_arm_l", "torso", (0.4, 0.44), 0.15, 105, (45, 160)),
    _bone("forearm_l", "upper_arm_l", (0.34, 0.57), 0.14, 80, (15, 150)),
    _bone("hand_l", "forearm_l", (0.35, 0.7), 0.06, 90, (30, 145)),
    _bone("upper_arm_r", "torso", (0.6, 0.44), 0.15, 75, (20, 135)),
    _bone("forearm_r", "upper_arm_r", (0.66, 0.57), 0.14, 100, (30, 165)),
    _bone("hand_r", "forearm_r", (0.65, 0.7), 0.06, 90, (35, 150)),
    _bone("thigh_l", "pelvis", (0.45, 0.66), 0.18, 90, (45, 135)),
    _bone("calf_l", "thigh_l", (0.43, 0.82), 0.16, 90, (35, 145)),
    _bone("foot_l", "calf_l", (0.43, 0.94), 0.09, 0, (-40, 35)),
    _bone("thigh_r", "pelvis", (0.55, 0.66), 0.18, 90, (45, 135)),
    _bone("calf_r", "thigh_r", (0.57, 0.82), 0.16, 90, (35, 145)),
    _bone("foot_r", "calf_r", (0.57, 0.94), 0.09, 0, (-40, 35)),
    _bone("hair_back", "head", (0.5, 0.24), 0.18, 90, (65, 115)),
    _bone("skirt", "pelvis", (0.5, 0.62), 0.18, 90, (70, 110)),
]

CAT_PARTS = [
    ("tail", "tail_01", (0.72, 0.64), -3),
    ("hind_leg_l", "hind_leg_l", (0.32, 0.67), -2),
    ("hind_leg_r", "hind_leg_r", (0.68, 0.67), -2),
    ("body", "torso", (0.5, 0.64), 0),
    ("front_leg_l", "front_leg_l", (0.41, 0.66), 1),
    ("front_leg_r", "front_leg_r", (0.57, 0.66), 1),
    ("head", "head", (0.5, 0.38), 2),
    ("ear_l", "ear_l", (0.42, 0.23), 3),
    ("ear_r", "ear_r", (0.58, 0.23), 3),
    ("eyes_open", "head", (0.5, 0.36), 4),
    ("eyes_closed", "head", (0.5, 0.36), 4),
]

HUMAN_PARTS = [
    ("hair_back", "hair_back", (0.5, 0.24), -4),
    ("arm_l", "upper_arm_l", (0.4, 0.44), -2),
    ("leg_l", "thigh_l", (0.45, 0.66), -2),
    ("torso", "torso", (0.5, 0.48), 0),
    ("skirt", "skirt", (0.5, 0.62), 1),
    ("leg_r", "thigh_r", (0.55, 0.66), 1),
    ("arm_r", "upper_arm_r", (0.6, 0.44), 2),
    ("head", "head", (0.5, 0.28), 3),
    ("hair_front", "head", (0.5, 0.2), 4),
    ("eyes_open", "head", (0.5, 0.29), 5),
    ("eyes_closed", "head", (0.5, 0.29), 5),
]


def rig_template(subject_kind: str) -> dict:
    human = subject_kind == "human_avatar"
    return {
        "schema_version": 1,
        "template_id": "human-chibi-a" if human else "cat-short-hair-a",
        "template_version": "1.0.0",
        "subject_kind": "human_avatar" if human else "pet_cat",
        "bones": deepcopy(HUMAN_BONES if human else CAT_BONES),
        "anchors": {
            "foot": [0.5, 0.94 if human else 0.91],
            "drag": [0.5, 0.34],
            "center": [0.5, 0.55],
            "head": [0.5, 0.27 if human else 0.34],
        },
    }


def layer_bundle(subject_kind: str) -> dict:
    human = subject_kind == "human_avatar"
    parts = HUMAN_PARTS if human else CAT_PARTS
    return {
        "schema_version": 1,
        "subject_kind": "human_avatar" if human else "pet_cat",
        "canvas": {"width": 256, "height": 256},
        "parts": [
            {
                "id": part_id,
                "bone": bone,
                "pivot": list(pivot),
                "z_index": z_index,
                "source": "procedural_mock",
            }
            for part_id, bone, pivot, z_index in parts
        ],
    }
