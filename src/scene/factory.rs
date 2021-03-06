pub mod shader {
    use graphics::errors::*;
    use graphics::*;
    use resource::Location;

    pub const PBR: &str = "__Core/Scene/Shader/PBR";
    pub const PHONG: &str = "__Core/Scene/Shader/PHONG";
    pub const UNDEFINED: &str = "__Core/Scene/Shader/UNDEFINED";
    pub const COLOR: &str = "__Core/Scene/Shader/COLOR";

    pub fn pbr(video: &GraphicsSystemShared) -> Result<ShaderHandle> {
        let location = Location::shared(0, PBR);
        if let Some(shader) = video.lookup_shader_from(location) {
            return Ok(shader);
        }

        let attributes = AttributeLayout::build()
            .with(Attribute::Position, 4)
            .with(Attribute::Normal, 4)
            .with(Attribute::Texcoord0, 2)
            .finish();

        let mut render_state = RenderState::default();
        render_state.depth_write = true;
        render_state.depth_test = Comparison::LessOrEqual;

        let mut setup = ShaderSetup::default();
        setup.render_state = render_state;
        setup.layout = attributes;
        setup.vs = include_str!("assets/pbr.vs").to_owned();
        setup.fs = include_str!("assets/pbr.fs").to_owned();

        let uvs = [
            ("u_MVPMatrix", UniformVariableType::Matrix4f),
            ("u_ModelViewMatrix", UniformVariableType::Matrix4f),
            ("u_NormalMatrix", UniformVariableType::Matrix4f),
        ];

        for &(field, tt) in &uvs {
            setup.uniform_variables.insert(field.into(), tt);
        }

        video.create_shader(location, setup)
    }

    pub fn phong(video: &GraphicsSystemShared) -> Result<ShaderHandle> {
        let location = Location::shared(0, PHONG);
        if let Some(shader) = video.lookup_shader_from(location) {
            return Ok(shader);
        }

        let attributes = AttributeLayout::build()
            .with(Attribute::Position, 3)
            .with(Attribute::Normal, 3)
            .with(Attribute::Color0, 4)
            .finish();

        let mut render_state = RenderState::default();
        render_state.depth_write = true;
        render_state.depth_test = Comparison::LessOrEqual;
        render_state.cull_face = CullFace::Back;

        let mut setup = ShaderSetup::default();
        setup.render_state = render_state;
        setup.layout = attributes;
        setup.vs = include_str!("assets/phong.vs").to_owned();
        setup.fs = include_str!("assets/phong.fs").to_owned();

        let uvs = [
            ("u_MVPMatrix", UniformVariableType::Matrix4f),
            ("u_ModelViewMatrix", UniformVariableType::Matrix4f),
            ("u_NormalMatrix", UniformVariableType::Matrix4f),
            ("u_DirLightEyeDir", UniformVariableType::Vector3f),
            ("u_DirLightColor", UniformVariableType::Vector3f),
            ("u_PointLightEyePos[0]", UniformVariableType::Vector3f),
            ("u_PointLightColor[0]", UniformVariableType::Vector3f),
            ("u_PointLightAttenuation[0]", UniformVariableType::Vector3f),
            ("u_PointLightEyePos[1]", UniformVariableType::Vector3f),
            ("u_PointLightColor[1]", UniformVariableType::Vector3f),
            ("u_PointLightAttenuation[1]", UniformVariableType::Vector3f),
            ("u_PointLightEyePos[2]", UniformVariableType::Vector3f),
            ("u_PointLightColor[2]", UniformVariableType::Vector3f),
            ("u_PointLightAttenuation[2]", UniformVariableType::Vector3f),
            ("u_PointLightEyePos[3]", UniformVariableType::Vector3f),
            ("u_PointLightColor[3]", UniformVariableType::Vector3f),
            ("u_PointLightAttenuation[3]", UniformVariableType::Vector3f),
            ("u_Ambient", UniformVariableType::Vector3f),
            ("u_Diffuse", UniformVariableType::Vector3f),
            ("u_Specular", UniformVariableType::Vector3f),
            ("u_Shininess", UniformVariableType::F32),
        ];

        for &(field, tt) in &uvs {
            setup.uniform_variables.insert(field.into(), tt);
        }

        video.create_shader(location, setup)
    }

    pub fn color(video: &GraphicsSystemShared) -> Result<ShaderHandle> {
        let location = Location::shared(0, COLOR);
        if let Some(shader) = video.lookup_shader_from(location) {
            return Ok(shader);
        }

        let attributes = AttributeLayout::build()
            .with(Attribute::Position, 3)
            .finish();

        let mut render_state = RenderState::default();
        render_state.depth_write = true;
        render_state.depth_test = Comparison::LessOrEqual;
        render_state.cull_face = CullFace::Back;

        let mut setup = ShaderSetup::default();
        setup.render_state = render_state;
        setup.layout = attributes;
        setup.vs = include_str!("assets/color.vs").to_owned();
        setup.fs = include_str!("assets/color.fs").to_owned();

        let uvs = [
            ("u_MVPMatrix", UniformVariableType::Matrix4f),
            ("u_Color", UniformVariableType::Vector4f),
        ];

        for &(field, tt) in &uvs {
            setup.uniform_variables.insert(field.into(), tt);
        }

        video.create_shader(location, setup)
    }

    pub fn undefined(video: &GraphicsSystemShared) -> Result<ShaderHandle> {
        let location = Location::shared(0, UNDEFINED);
        if let Some(shader) = video.lookup_shader_from(location) {
            return Ok(shader);
        }

        let attributes = AttributeLayout::build()
            .with(Attribute::Position, 3)
            .finish();

        let mut render_state = RenderState::default();
        render_state.depth_write = true;
        render_state.depth_test = Comparison::LessOrEqual;
        render_state.cull_face = CullFace::Back;

        let mut setup = ShaderSetup::default();
        setup.render_state = render_state;
        setup.layout = attributes;
        setup.vs = include_str!("assets/undefined.vs").to_owned();
        setup.fs = include_str!("assets/undefined.fs").to_owned();

        let uvs = [("u_MVPMatrix", UniformVariableType::Matrix4f)];

        for &(field, tt) in &uvs {
            setup.uniform_variables.insert(field.into(), tt);
        }

        video.create_shader(location, setup)
    }
}

pub mod mesh {
    use graphics::errors::*;
    use graphics::*;
    use resource::Location;

    impl_vertex! {
        PrimitiveVertex {
            position => [Position; Float; 3; false],
            color => [Color0; UByte; 4; true],
            texcoord => [Texcoord0; Float; 2; false],
            normal => [Normal; Float; 3; false],
        }
    }

    pub const CUBE: &str = "__Core/Scene/Mesh/CUBE";

    pub fn cube(video: &GraphicsSystemShared) -> Result<MeshHandle> {
        let location = Location::shared(0, CUBE);
        if let Some(cube) = video.lookup_mesh_from(location) {
            return Ok(cube);
        }

        let color = [155, 155, 155, 255];
        let texcoords = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

        let points = [
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
        ];

        let normals = [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
        ];

        let verts = [
            PrimitiveVertex::new(points[0], color, texcoords[0], normals[0]),
            PrimitiveVertex::new(points[1], color, texcoords[1], normals[0]),
            PrimitiveVertex::new(points[2], color, texcoords[2], normals[0]),
            PrimitiveVertex::new(points[3], color, texcoords[3], normals[0]),
            PrimitiveVertex::new(points[1], color, texcoords[0], normals[1]),
            PrimitiveVertex::new(points[5], color, texcoords[1], normals[1]),
            PrimitiveVertex::new(points[6], color, texcoords[2], normals[1]),
            PrimitiveVertex::new(points[2], color, texcoords[3], normals[1]),
            PrimitiveVertex::new(points[5], color, texcoords[0], normals[2]),
            PrimitiveVertex::new(points[4], color, texcoords[1], normals[2]),
            PrimitiveVertex::new(points[7], color, texcoords[2], normals[2]),
            PrimitiveVertex::new(points[6], color, texcoords[3], normals[2]),
            PrimitiveVertex::new(points[4], color, texcoords[0], normals[3]),
            PrimitiveVertex::new(points[0], color, texcoords[1], normals[3]),
            PrimitiveVertex::new(points[3], color, texcoords[2], normals[3]),
            PrimitiveVertex::new(points[7], color, texcoords[3], normals[3]),
            PrimitiveVertex::new(points[3], color, texcoords[0], normals[4]),
            PrimitiveVertex::new(points[2], color, texcoords[1], normals[4]),
            PrimitiveVertex::new(points[6], color, texcoords[2], normals[4]),
            PrimitiveVertex::new(points[7], color, texcoords[3], normals[4]),
            PrimitiveVertex::new(points[4], color, texcoords[0], normals[5]),
            PrimitiveVertex::new(points[5], color, texcoords[1], normals[5]),
            PrimitiveVertex::new(points[1], color, texcoords[2], normals[5]),
            PrimitiveVertex::new(points[0], color, texcoords[3], normals[5]),
        ];

        #[cfg_attr(rustfmt, rustfmt_skip)]
        let idxes = [
            0, 1, 2, 2, 3, 0,
            4, 5, 6, 6, 7, 4,
            8, 9, 10, 10, 11, 8,
            12, 13, 14, 14, 15, 12,
            16, 17, 18, 18, 19, 16,
            20, 21, 22, 22, 23, 20,
        ];

        let mut setup = MeshSetup::default();
        setup.layout = PrimitiveVertex::layout();
        setup.num_verts = verts.len();
        setup.num_idxes = idxes.len();
        setup.sub_mesh_offsets.push(0);

        let vbytes = PrimitiveVertex::as_bytes(&verts);
        let ibytes = IndexFormat::as_bytes::<u16>(&idxes);
        video.create_mesh(location, setup, vbytes, ibytes)
    }
}
