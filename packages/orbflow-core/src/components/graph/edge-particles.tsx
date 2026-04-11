export interface EdgeParticlesProps {
  path: string;
  color?: string;
  primaryRadius?: number;
  secondaryRadius?: number;
  duration?: string;
}

/** Animated SVG particles that travel along an edge path. Must be rendered inside an SVG context. */
export function EdgeParticles({
  path,
  color = "#4A9AAF",
  primaryRadius = 4,
  secondaryRadius = 3,
  duration = "1.5s",
}: EdgeParticlesProps): React.ReactNode {
  return (
    <>
      <circle r={primaryRadius} fill={color} opacity={0.9}>
        <animateMotion
          dur={duration}
          repeatCount="indefinite"
          path={path}
        />
      </circle>
      <circle r={secondaryRadius} fill={color} opacity={0.5}>
        <animateMotion
          dur={duration}
          begin={`${parseFloat(duration) / 2}s`}
          repeatCount="indefinite"
          path={path}
        />
      </circle>
    </>
  );
}
