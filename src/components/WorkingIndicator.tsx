import type { WorkPhase } from '../lib/reasoning';

const LABEL: Record<WorkPhase, string> = {
  writing: 'Working',
  thinking: 'Thinking it through',
  tool: 'Calling a tool',
};

/** Shown under the reply while the provider is still working. */
export default function WorkingIndicator({ phase }: { phase: WorkPhase }) {
  return (
    <p className="working-indicator" role="status">
      {LABEL[phase]}
      <span className="working-dots" aria-hidden="true">
        <i />
        <i />
        <i />
      </span>
    </p>
  );
}
