extends Control

@onready var state: Node = $State

func _ready() -> void:
	$Panel/Hit.pressed.connect(_hit)
	$Panel/Reset.pressed.connect(_reset)
	render_state()

func _hit() -> void:
	state.call("Apply", 1)
	render_state()

func _reset() -> void:
	state.call("Reset")
	render_state()

func render_state() -> void:
	$Panel/Value.text = "%d / %d" % [state.get("Value"), state.get("Limit")]
