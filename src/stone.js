// Procedural wear. Everything is seeded, so a given document always breaks the same way:
// each artifact keeps its own fractures between openings.

const SVG_NS = "http://www.w3.org/2000/svg";

export function seedFrom(text) {
  let hash = 2166136261;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function generator(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// One side of the outline, walked from its own start corner. Every side has its own random
// stream, so resizing the window extends the edge instead of reshuffling all the damage.
function chippedSide(length, random, { roughness, chipDepth, chipChance, margin }) {
  const points = [];
  let along = margin + random() * 8;
  let chip = 0;
  let chipPeak = 0;
  let chipSpan = 0;
  while (along < length - margin) {
    let inward = random() * roughness;
    if (chip === 0 && random() < chipChance) {
      chipSpan = 2 + Math.floor(random() * 4);
      chip = chipSpan;
      chipPeak = chipDepth * (0.4 + random() * 0.6);
    }
    if (chip > 0) {
      const progress = 1 - chip / chipSpan;
      // Steep where the flake came off, shallower where it ran out.
      inward += chipPeak * Math.pow(1 - progress, 0.7);
      chip--;
    }
    points.push([along, inward]);
    along += chip > 0 ? 3 + random() * 7 : 8 + random() * 24;
  }
  return points;
}

export function chippedOutline(width, height, seed, options = {}) {
  const shape = {
    roughness: 1.6,
    chipDepth: 7,
    chipChance: 0.08,
    cornerBite: 12,
    bigBreak: 0,
    ...options,
  };
  const master = generator(seed);
  const sideSeeds = [0, 1, 2, 3].map(() => Math.floor(master() * 2 ** 32));
  const bites = [0, 1, 2, 3].map(() => shape.cornerBite * (0.35 + master() * 0.65));
  if (shape.bigBreak > 0) {
    bites[Math.floor(master() * 4)] = shape.bigBreak * (0.55 + master() * 0.45);
  }

  // Corner order: top-left, top-right, bottom-right, bottom-left.
  const sides = [
    { from: [0, 0], along: [1, 0], inward: [0, 1], length: width },
    { from: [width, 0], along: [0, 1], inward: [-1, 0], length: height },
    { from: [width, height], along: [-1, 0], inward: [0, -1], length: width },
    { from: [0, height], along: [0, -1], inward: [1, 0], length: height },
  ];

  const outline = [];
  sides.forEach((side, index) => {
    const random = generator(sideSeeds[index]);
    const startBite = bites[index];
    const endBite = bites[(index + 1) % 4];
    const at = (distance, depth) => [
      side.from[0] + side.along[0] * distance + side.inward[0] * depth,
      side.from[1] + side.along[1] * distance + side.inward[1] * depth,
    ];
    outline.push(at(startBite * 0.1, startBite * (0.9 + random() * 0.1)));
    outline.push(at(startBite * 0.55, startBite * 0.4 * random() + 0.5));
    for (const [distance, depth] of chippedSide(side.length, random, {
      ...shape,
      margin: Math.max(startBite, endBite, 6),
    })) {
      if (distance > startBite && distance < side.length - endBite) {
        outline.push(at(distance, depth));
      }
    }
    outline.push(at(side.length - endBite * 0.8, endBite * 0.2 * random()));
  });
  return outline;
}

export function polygonCss(points) {
  return `polygon(${points.map(([x, y]) => `${x.toFixed(1)}px ${y.toFixed(1)}px`).join(",")})`;
}

function pathD(points) {
  return `M${points.map(([x, y]) => `${x.toFixed(1)} ${y.toFixed(1)}`).join("L")}Z`;
}

// A fracture as a filled, tapering sliver: wide at the rim, closing to nothing.
function fracture(start, angle, length, startWidth, random) {
  const spine = [start];
  let [x, y] = start;
  let heading = angle;
  let travelled = 0;
  while (travelled < length) {
    const step = 7 + random() * 19;
    heading += (random() - 0.5) * 1.05;
    // Pull back toward the main direction, or the crack curls around itself.
    heading += (angle - heading) * 0.28;
    x += Math.cos(heading) * step;
    y += Math.sin(heading) * step;
    travelled += step;
    spine.push([x, y]);
  }

  const left = [];
  const right = [];
  spine.forEach(([px, py], index) => {
    const next = spine[Math.min(index + 1, spine.length - 1)];
    const previous = spine[Math.max(index - 1, 0)];
    const dir = Math.atan2(next[1] - previous[1], next[0] - previous[0]);
    const remaining = 1 - index / (spine.length - 1);
    const half = (startWidth * Math.pow(remaining, 1.3) * (0.65 + random() * 0.7)) / 2;
    left.push([px + Math.cos(dir + Math.PI / 2) * half, py + Math.sin(dir + Math.PI / 2) * half]);
    right.push([px - Math.cos(dir + Math.PI / 2) * half, py - Math.sin(dir + Math.PI / 2) * half]);
  });
  return { outline: [...left, ...right.reverse()], spine };
}

export function drawFractures(svg, width, height, seed, { density = 1, reach = 0.3, rim = null } = {}) {
  const random = generator(seed);
  const shortest = Math.min(width, height);
  const count = Math.max(2, Math.round(((width + height) / 420) * density));
  const shapes = [];

  for (let i = 0; i < count; i++) {
    const edge = Math.floor(random() * 4);
    const position = 0.06 + random() * 0.88;
    const start = [
      [position * width, 0],
      [width, position * height],
      [position * width, height],
      [0, position * height],
    ][edge];
    const inwardAngle = [Math.PI / 2, Math.PI, -Math.PI / 2, 0][edge];
    const angle = inwardAngle + (random() - 0.5) * 1.3;
    const long = random() < 0.2;
    const length = shortest * reach * (long ? 0.9 + random() * 0.5 : 0.25 + random() * 0.45);
    const crack = fracture(start, angle, length, 1.6 + random() * 2.2, random);
    shapes.push(crack.outline);

    const branches = long ? 2 : random() < 0.45 ? 1 : 0;
    for (let b = 0; b < branches; b++) {
      const from = crack.spine[Math.floor(crack.spine.length * (0.25 + random() * 0.4))];
      const branch = fracture(from, angle + (random() < 0.5 ? -1 : 1) * (0.5 + random() * 0.6), length * (0.2 + random() * 0.3), 0.9 + random() * 0.7, random);
      shapes.push(branch.outline);
    }
  }

  const d = shapes.map(pathD).join("");
  const nodes = [];
  // Lit lip first, dark cut on top: reads as depth, with light from the upper left.
  nodes.push(svgPath(d, "crack-lit", "translate(0.8 0.9)"));
  nodes.push(svgPath(d, "crack-cut"));
  if (rim) {
    const rimPath = pathD(rim);
    nodes.push(svgPath(rimPath, "rim-shade", "translate(-0.6 -0.8)"));
    nodes.push(svgPath(rimPath, "rim-lit", "translate(0.6 0.8)"));
  }
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.replaceChildren(...nodes);
}

function svgPath(d, className, transform) {
  const path = document.createElementNS(SVG_NS, "path");
  path.setAttribute("d", d);
  path.setAttribute("class", className);
  if (transform) path.setAttribute("transform", transform);
  return path;
}
