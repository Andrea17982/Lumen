import { useState } from 'react';
import { advance } from './counter';
export default function CounterView() {
  const [value, setValue] = useState(0);
  const limit = 3;
  return (
    <main>
      <h1>Counter</h1>
      <output aria-label="Value">
        {value} / {limit}
      </output>
      <button onClick={() => setValue(advance(value, 1, limit))}>Hit</button>
      <button onClick={() => setValue(0)}>Reset</button>
    </main>
  );
}
