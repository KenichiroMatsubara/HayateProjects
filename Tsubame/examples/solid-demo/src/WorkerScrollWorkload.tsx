const BANDS = [
  '#0f766e',
  '#2563eb',
  '#7c3aed',
  '#c026d3',
  '#e11d48',
] as const;

/**
 * Deterministic Canvas acceptance scene. It deliberately avoids text and image setup so the named
 * workload measures input → Tsubame commit → shared frame pipeline → Vello/WebGPU → present.
 */
export function WorkerScrollWorkload() {
  return (
    <view
      style={{
        width: '100%',
        height: '100%',
        backgroundColor: '#020617',
      }}
    >
      <scroll-view
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
          backgroundColor: '#020617',
        }}
      >
        {BANDS.map((color, index) => (
          <view
            style={{
              width: '100%',
              height: 180 + (index % 3) * 30,
              flexShrink: 0,
              backgroundColor: color,
            }}
          />
        ))}
      </scroll-view>
    </view>
  );
}
