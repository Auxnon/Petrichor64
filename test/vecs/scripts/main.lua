-- What silt's `vector` feature gives Lua. Enabled via the silt-lua dependency in
-- Cargo.toml; without it `vec3` is nil and everything below fails.
--
--     Petrichor64 test/vecs
--
-- vec2/vec3/vec4 are first-class VM values (glam-backed), not tables or userdata:
-- they carry operators, field access and a method library, which is what makes the
-- modeller's normals and extrusion readable.
step = 0
fails = 0

function ok(label, got, want)
	if got == want then
		cout("ok", label, got)
	else
		fails = fails + 1
		cout("FAIL", label, "got", got, "want", want)
	end
end

function main()
	cout("vector probe; vec library is a", type(vec))
end

function loop()
	step = step + 1
	if step == 10 then
		local a = vec3(1, 2, 3)
		ok("type name", type(a), "vec3")
		ok("field x", a.x, 1.0)
		ok("field z", a.z, 3.0)
		cout("tostring", tostring(a))
	elseif step == 20 then
		local a = vec3(1, 2, 3)
		local b = vec3(4, 5, 6)
		ok("add", tostring(a + b), "vec3(5, 7, 9)")
		ok("sub", tostring(b - a), "vec3(3, 3, 3)")
		ok("scale", tostring(a * 2), "vec3(2, 4, 6)")
		ok("equality", vec3(1, 2, 3) == vec3(1, 2, 3), true)
	elseif step == 30 then
		ok("length", vec3(3, 4, 0):length(), 5.0)
		ok("distance", vec3(0, 0, 0):distance(vec3(0, 3, 4)), 5.0)
		ok("dot", vec3(1, 0, 0):dot(vec3(1, 0, 0)), 1.0)
		ok("normalize", tostring(vec3(0, 0, 2):normalize()), "vec3(0, 0, 1)")
		ok("cross", tostring(vec3(1, 0, 0):cross(vec3(0, 1, 0))), "vec3(0, 0, 1)")
	elseif step == 40 then
		-- Components feed engine calls that want plain numbers.
		local p = vec3(0, 8, 0)
		local e = make("example", p.x, p.y, p.z)
		ok("spawn from components", e ~= nil, true)
	elseif step == 50 then
		-- mod() builds a mesh from Lua; both forms require a texture at `t`.
		-- `t` is required for the quad form too; without it nothing is built (it used
		-- to fail silently and report success).
		mod("probequad", {
			q = { { 0, 0, 0 }, { 1, 0, 0 }, { 1, 0, 1 }, { 0, 0, 1 } },
			t = { "example" },
		})
		ok("quad mesh", make("probequad", 0, 6, 0) ~= nil, true)
		ok("quad mesh exists", #gmod("probequad"), 1)
		mod("probetri", {
			v = { { 0, 0, 0 }, { 1, 0, 0 }, { 0, 0, 1 } },
			i = { 0, 1, 2 },
			u = { { 0, 0 }, { 1, 0 }, { 0, 1 } },
			t = { "example" },
		})
		ok("vertex mesh", make("probetri", 2, 6, 0) ~= nil, true)
	elseif step == 60 then
		cout("done, failures:", fails)
	end
end

function draw()
end
