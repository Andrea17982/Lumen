extends SceneTree

var failures: Array[String] = []

func _initialize() -> void:
	call_deferred("_run")

func verify(value: bool, label: String) -> void:
	if not value:
		failures.append(label)
		push_error(label)

func _run() -> void:
	var packed = load("res://Main.tscn")
	if packed == null:
		quit(2)
		return
	var scene = packed.instantiate()
	root.add_child(scene)
	await process_frame
	var state = scene.get_node("State")
	var hit = scene.get_node("Panel/Hit")
	var reset = scene.get_node("Panel/Reset")
	var label = scene.get_node("Panel/Value")
	verify(int(state.get("Value")) == 0 and label.text == "0 / 3", "initial value and label")
	for i in range(1, 4):
		hit.pressed.emit()
		verify(int(state.get("Value")) == i and label.text == "%d / 3" % i, "hit %d and label" % i)
	verify(hit.disabled, "disable at upper bound")
	reset.pressed.emit()
	verify(int(state.get("Value")) == 0 and label.text == "0 / 3" and not hit.disabled, "reset state/label/button")
	state.call("Apply", 100)
	verify(int(state.get("Value")) == 3, "C# upper clamp")
	state.call("Apply", -100)
	verify(int(state.get("Value")) == 0, "C# lower clamp")
	state.set("Limit", 0)
	state.call("Reset")
	state.call("Apply", 10)
	verify(int(state.get("Value")) == 0, "zero limit clamp")
	scene.queue_free()
	print(JSON.stringify({"passed": failures.is_empty(), "failures": failures, "checks": 9}))
	quit(0 if failures.is_empty() else 1)
