# ferrostep (Python)

Python bindings for [FerroStep](https://github.com/Artificial-Humanity/FerroStep),
a data-driven state-machine referee for database-ledger multi-agent loops.

```python
from ferrostep import Engine

engine = Engine(workflow)  # dict; validated on construction
decision = engine.authorize(state="awaiting_worker",
                            counters={"agent_passes": 0},
                            role="worker", to="working")
# {'kind': 'allow', 'to': 'working', 'counter_updates': {'agent_passes': 1}}
```

See the repository README for the workflow-definition format and the design
contract (atomic writes, spend-on-entry counters).
