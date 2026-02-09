//! Nested Command Buffer Support
//!
//! VK_EXT_nested_command_buffer enables hierarchical command buffer execution:
//! - Execute secondary command buffers from other secondary command buffers
//! - Better command buffer organization
//! - Improved parallelism for complex scenes
//! - Useful for multi-pass rendering

use ash::vk;

/// Nested command buffer capabilities.
#[derive(Debug, Clone, Default)]
pub struct NestedCommandBufferCapabilities {
    /// Whether nested command buffers are supported.
    pub supported: bool,
    /// Maximum nesting level.
    pub max_nesting_level: u32,
}

/// Query nested command buffer capabilities.
pub fn query_capabilities(ctx: &super::context::VulkanContext) -> NestedCommandBufferCapabilities {
    let mut nested_features = vk::PhysicalDeviceNestedCommandBufferFeaturesEXT::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default()
        .push_next(&mut nested_features);

    unsafe {
        ctx.instance.get_physical_device_features2(ctx.physical_device, &mut features2);
    }

    let _ = features2;
    let supported = nested_features.nested_command_buffer == vk::TRUE;

    if !supported {
        return NestedCommandBufferCapabilities::default();
    }

    let mut nested_props = vk::PhysicalDeviceNestedCommandBufferPropertiesEXT::default();
    let mut props2 = vk::PhysicalDeviceProperties2::default()
        .push_next(&mut nested_props);

    unsafe {
        ctx.instance.get_physical_device_properties2(ctx.physical_device, &mut props2);
    }

    NestedCommandBufferCapabilities {
        supported,
        max_nesting_level: nested_props.max_command_buffer_nesting_level,
    }
}

/// Command buffer hierarchy node.
#[derive(Debug)]
pub struct CommandBufferNode {
    /// The command buffer.
    pub command_buffer: vk::CommandBuffer,
    /// Nesting level (0 = primary).
    pub level: u32,
    /// Child command buffers.
    pub children: Vec<CommandBufferNode>,
    /// Name for debugging.
    pub name: String,
}

impl CommandBufferNode {
    /// Create a new node.
    pub fn new(command_buffer: vk::CommandBuffer, level: u32, name: &str) -> Self {
        Self {
            command_buffer,
            level,
            children: Vec::new(),
            name: name.to_string(),
        }
    }

    /// Add a child command buffer.
    pub fn add_child(&mut self, child: CommandBufferNode) {
        self.children.push(child);
    }

    /// Get total command buffer count in tree.
    pub fn total_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_count()).sum::<usize>()
    }

    /// Get maximum depth of tree.
    pub fn max_depth(&self) -> u32 {
        if self.children.is_empty() {
            self.level
        } else {
            self.children.iter().map(|c| c.max_depth()).max().unwrap_or(self.level)
        }
    }
}

/// Builder for hierarchical command buffer recording.
pub struct HierarchicalCommandBuilder {
    /// Command pool.
    pool: vk::CommandPool,
    /// Allocated command buffers.
    buffers: Vec<vk::CommandBuffer>,
    /// Root node.
    root: Option<CommandBufferNode>,
    /// Maximum nesting level.
    max_level: u32,
}

impl HierarchicalCommandBuilder {
    /// Create a new builder.
    pub fn new(pool: vk::CommandPool, max_level: u32) -> Self {
        Self {
            pool,
            buffers: Vec::new(),
            root: None,
            max_level,
        }
    }

    /// Allocate a primary command buffer as root.
    pub fn allocate_primary(
        &mut self,
        ctx: &super::context::VulkanContext,
        name: &str,
    ) -> Result<vk::CommandBuffer, String> {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let buffers = unsafe {
            ctx.device.allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate primary command buffer: {:?}", e))?
        };

        let cmd = buffers[0];
        self.buffers.push(cmd);
        self.root = Some(CommandBufferNode::new(cmd, 0, name));

        Ok(cmd)
    }

    /// Allocate a secondary command buffer.
    pub fn allocate_secondary(
        &mut self,
        ctx: &super::context::VulkanContext,
        name: &str,
        level: u32,
    ) -> Result<vk::CommandBuffer, String> {
        if level > self.max_level {
            return Err(format!("Nesting level {} exceeds maximum {}", level, self.max_level));
        }

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.pool)
            .level(vk::CommandBufferLevel::SECONDARY)
            .command_buffer_count(1);

        let buffers = unsafe {
            ctx.device.allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate secondary command buffer: {:?}", e))?
        };

        let cmd = buffers[0];
        self.buffers.push(cmd);

        Ok(cmd)
    }

    /// Begin recording a secondary command buffer.
    pub fn begin_secondary(
        ctx: &super::context::VulkanContext,
        cmd: vk::CommandBuffer,
        inheritance: &vk::CommandBufferInheritanceInfo,
    ) -> Result<(), String> {
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .inheritance_info(inheritance);

        unsafe {
            ctx.device.begin_command_buffer(cmd, &begin_info)
                .map_err(|e| format!("Failed to begin secondary command buffer: {:?}", e))?;
        }

        Ok(())
    }

    /// Execute secondary command buffers from within another secondary command buffer.
    pub fn execute_nested(
        ctx: &super::context::VulkanContext,
        parent: vk::CommandBuffer,
        children: &[vk::CommandBuffer],
    ) {
        if !children.is_empty() {
            unsafe {
                ctx.device.cmd_execute_commands(parent, children);
            }
        }
    }

    /// End recording a command buffer.
    pub fn end(ctx: &super::context::VulkanContext, cmd: vk::CommandBuffer) -> Result<(), String> {
        unsafe {
            ctx.device.end_command_buffer(cmd)
                .map_err(|e| format!("Failed to end command buffer: {:?}", e))?;
        }
        Ok(())
    }

    /// Get all allocated command buffers.
    pub fn command_buffers(&self) -> &[vk::CommandBuffer] {
        &self.buffers
    }

    /// Free all command buffers.
    pub fn free_all(&mut self, ctx: &super::context::VulkanContext) {
        if !self.buffers.is_empty() {
            unsafe {
                ctx.device.free_command_buffers(self.pool, &self.buffers);
            }
            self.buffers.clear();
            self.root = None;
        }
    }
}

/// Example: Render pass hierarchy for deferred rendering.
#[derive(Debug)]
pub struct DeferredRenderHierarchy {
    /// G-buffer pass command buffers.
    pub gbuffer_pass: Vec<vk::CommandBuffer>,
    /// Lighting pass command buffers.
    pub lighting_pass: Vec<vk::CommandBuffer>,
    /// Post-process pass command buffers.
    pub post_process_pass: Vec<vk::CommandBuffer>,
}

/// Create a typical deferred rendering hierarchy.
pub fn create_deferred_hierarchy(
    ctx: &super::context::VulkanContext,
    builder: &mut HierarchicalCommandBuilder,
    batch_count: u32,
) -> Result<DeferredRenderHierarchy, String> {
    let mut gbuffer_pass = Vec::with_capacity(batch_count as usize);
    let mut lighting_pass = Vec::with_capacity(batch_count as usize);
    let mut post_process_pass = Vec::new();

    // G-buffer batches (level 1)
    for i in 0..batch_count {
        let cmd = builder.allocate_secondary(ctx, &format!("gbuffer_batch_{}", i), 1)?;
        gbuffer_pass.push(cmd);
    }

    // Lighting batches (level 1)
    for i in 0..batch_count {
        let cmd = builder.allocate_secondary(ctx, &format!("lighting_batch_{}", i), 1)?;
        lighting_pass.push(cmd);
    }

    // Post-process (level 1, single)
    let post_cmd = builder.allocate_secondary(ctx, "post_process", 1)?;
    post_process_pass.push(post_cmd);

    Ok(DeferredRenderHierarchy {
        gbuffer_pass,
        lighting_pass,
        post_process_pass,
    })
}

/// Inheritance info builder for nested command buffers.
pub struct InheritanceBuilder {
    render_pass: vk::RenderPass,
    subpass: u32,
    framebuffer: vk::Framebuffer,
    occlusion_query_enable: bool,
    query_flags: vk::QueryControlFlags,
    pipeline_statistics: vk::QueryPipelineStatisticFlags,
}

impl Default for InheritanceBuilder {
    fn default() -> Self {
        Self {
            render_pass: vk::RenderPass::null(),
            subpass: 0,
            framebuffer: vk::Framebuffer::null(),
            occlusion_query_enable: false,
            query_flags: vk::QueryControlFlags::empty(),
            pipeline_statistics: vk::QueryPipelineStatisticFlags::empty(),
        }
    }
}

impl InheritanceBuilder {
    /// Create new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set render pass.
    pub fn render_pass(mut self, render_pass: vk::RenderPass) -> Self {
        self.render_pass = render_pass;
        self
    }

    /// Set subpass index.
    pub fn subpass(mut self, subpass: u32) -> Self {
        self.subpass = subpass;
        self
    }

    /// Set framebuffer.
    pub fn framebuffer(mut self, framebuffer: vk::Framebuffer) -> Self {
        self.framebuffer = framebuffer;
        self
    }

    /// Enable occlusion queries.
    pub fn occlusion_query(mut self, enable: bool, flags: vk::QueryControlFlags) -> Self {
        self.occlusion_query_enable = enable;
        self.query_flags = flags;
        self
    }

    /// Build the inheritance info.
    pub fn build(&self) -> vk::CommandBufferInheritanceInfo<'_> {
        vk::CommandBufferInheritanceInfo::default()
            .render_pass(self.render_pass)
            .subpass(self.subpass)
            .framebuffer(self.framebuffer)
            .occlusion_query_enable(self.occlusion_query_enable)
            .query_flags(self.query_flags)
            .pipeline_statistics(self.pipeline_statistics)
    }
}
