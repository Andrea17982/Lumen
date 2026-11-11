using Godot;

public partial class CounterState : Node
{
    [Export] public int Value { get; set; } = 0;
    [Export] public int Limit { get; set; } = 3;
    public void Apply(int step) { Value += step; }
    public void Reset() { Value = 0; }
}
