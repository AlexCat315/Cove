// WASI imports for the Rust WebGPU RHI. Object lifetimes and RHI descriptors
// come from Rust. This bridge binds them to the browser's JavaScript API.
export function createWebGpuBridge(wasm, canvas) {
  const decoder = new TextDecoder();
  const devices = new Map();
  let adapter, gpu, lost = false, fault;

  function complete(token, buffers = [], readback) {
    function retire(nanos) {
      for (const buffer of buffers) buffer.destroy();
      wasm().cove_webgpu_complete(token, nanos === undefined ? 0 : 1, nanos ?? 0n);
    }
    if (readback) {
      readback.mapAsync(GPUMapMode.READ).then(() => {
        const [start, end] = new BigUint64Array(readback.getMappedRange());
        readback.unmap();
        retire(end >= start ? end - start : undefined);
      }, () => gpu.queue.onSubmittedWorkDone().then(() => retire(), () => retire()));
    } else {
      gpu.queue.onSubmittedWorkDone().then(() => retire(), () => retire());
    }
  }

  function bindingResource(objects, resource) {
    if (resource.buffer !== undefined) {
      const binding = { buffer: objects.get(resource.buffer), offset: resource.offset };
      if (resource.size != null) binding.size = resource.size;
      return binding;
    }
    if (resource.texture !== undefined) return objects.get(resource.texture).createView();
    return objects.get(resource.sampler);
  }

  function encodePass(encoder, objects, record, transient) {
    if (record.type === "copy") {
      for (const [op, ...args] of record.body) {
        if (op === "texture") {
          const [src, dst, extent] = args;
          encoder.copyTextureToTexture({ texture: objects.get(src) }, { texture: objects.get(dst) }, extent);
        } else if (op === "buffer") {
          const [src, srcOffset, dst, dstOffset, size] = args;
          encoder.copyBufferToBuffer(objects.get(src), srcOffset, objects.get(dst), dstOffset, size);
        } else if (op === "upload") {
          const [buffer, offset, bytesPerRow, texture, mipLevel, origin, extent] = args;
          encoder.copyBufferToTexture({ buffer: objects.get(buffer), offset, bytesPerRow },
            { texture: objects.get(texture), mipLevel, origin }, extent);
        } else if (op === "download") {
          const [texture, buffer, offset, bytesPerRow, extent] = args;
          encoder.copyTextureToBuffer({ texture: objects.get(texture) },
            { buffer: objects.get(buffer), offset, bytesPerRow }, extent);
        }
      }
      return;
    }

    let pass;
    if (record.type === "render") {
      const colorAttachments = record.colors.map(({ texture, resolve, ...attachment }) => ({
        ...attachment, view: objects.get(texture).createView(),
        resolveTarget: resolve == null ? undefined : objects.get(resolve).createView(),
      }));
      const descriptor = { label: record.label, colorAttachments };
      if (record.depth) {
        const { texture, ...attachment } = record.depth;
        const depth = objects.get(texture);
        descriptor.depthStencilAttachment = { ...attachment, view: depth.createView() };
        if (depth.format.includes("stencil")) {
          descriptor.depthStencilAttachment.stencilReadOnly = true;
        }
      }
      pass = encoder.beginRenderPass(descriptor);
    } else {
      pass = encoder.beginComputePass();
    }

    let pipeline, appliedConstants;
    const values = new Map();
    function bindConstants() {
      const layout = pipeline.layout;
      if (!layout.constants.length || appliedConstants === layout) return;
      const entries = layout.constants.map(({ slot, size }) => {
        const buffer = gpu.createBuffer({ size, usage: GPUBufferUsage.UNIFORM, mappedAtCreation: true });
        const bytes = values.get(slot);
        if (bytes) new Uint8Array(buffer.getMappedRange()).set(bytes);
        buffer.unmap();
        transient.push(buffer);
        return { binding: slot, resource: { buffer } };
      });
      pass.setBindGroup(layout.constantGroup, gpu.createBindGroup({ layout: layout.constantLayout, entries }));
      appliedConstants = layout;
    }

    for (const [op, ...args] of record.body) {
      switch (op) {
        case "pipeline":
          pipeline = objects.get(args[0]);
          pass.setPipeline(pipeline.pipeline);
          break;
        case "binding": pass.setBindGroup(args[0], objects.get(args[1]), args[2]); break;
        case "constants": values.set(args[0], args[1]); appliedConstants = undefined; break;
        case "vertex": pass.setVertexBuffer(args[0], objects.get(args[1]), args[2]); break;
        case "index": pass.setIndexBuffer(objects.get(args[0]), args[1], args[2]); break;
        case "viewport": pass.setViewport(...args); break;
        case "scissor": pass.setScissorRect(...args); break;
        case "draw": bindConstants(); pass.draw(...args); break;
        case "indexed": bindConstants(); pass.drawIndexed(...args); break;
        case "indirect": {
          bindConstants();
          const [buffer, offset, count] = args;
          for (let i = 0; i < count; i++) pass.drawIndirect(objects.get(buffer), offset + 16 * i);
          break;
        }
        case "dispatch": bindConstants(); pass.dispatchWorkgroups(...args); break;
        case "dispatchIndirect": bindConstants(); pass.dispatchWorkgroupsIndirect(objects.get(args[0]), args[1]); break;
      }
    }
    pass.end();
  }

  function execute(objects, op, args, bytes) {
    switch (op) {
      case "check": break;
      case "configure": {
        if (args.canvas !== 1) throw new Error("Unknown WebGPU canvas token");
        canvas.width = args.width;
        canvas.height = args.height;
        const context = canvas.getContext("webgpu");
        context.configure({ device: gpu, format: args.format, alphaMode: args.alphaMode });
        objects.set(args.id, context);
        break;
      }
      case "acquire": objects.set(args.texture, objects.get(args.id).getCurrentTexture()); break;
      case "present": objects.delete(args.texture); complete(args.completion); break;
      case "buffer": objects.set(args.id, gpu.createBuffer(args.descriptor)); break;
      case "texture": objects.set(args.id, gpu.createTexture(args.descriptor)); break;
      case "sampler": objects.set(args.id, gpu.createSampler(args.descriptor)); break;
      case "shader": objects.set(args.id, {
        module: gpu.createShaderModule({ code: decoder.decode(bytes) }),
        entryPoint: args.entryPoint, constants: args.constants,
      }); break;
      case "bindingLayout": objects.set(args.id, gpu.createBindGroupLayout({ entries: args.entries })); break;
      case "bindingSet": objects.set(args.id, gpu.createBindGroup({
        layout: objects.get(args.layout),
        entries: args.entries.map(({ binding, resource }) => ({ binding, resource: bindingResource(objects, resource) })),
      })); break;
      case "pipelineLayout": {
        const layouts = args.sets.map(id => objects.get(id));
        const constantGroup = layouts.length;
        let constantLayout;
        if (args.constants.length) {
          constantLayout = gpu.createBindGroupLayout({ entries: args.constants.map(({ slot, visibility, size }) => ({
            binding: slot, visibility, buffer: { type: "uniform", minBindingSize: size },
          })) });
          layouts.push(constantLayout);
        }
        const layout = gpu.createPipelineLayout({ bindGroupLayouts: layouts });
        objects.set(args.id, { layout, constants: args.constants, constantGroup, constantLayout });
        break;
      }
      case "graphics": {
        const layout = objects.get(args.layout);
        const descriptor = { ...args.state, layout: layout.layout, vertex: { ...objects.get(args.vertex), buffers: args.buffers } };
        if (args.fragment != null) descriptor.fragment = { ...objects.get(args.fragment), targets: args.targets };
        objects.set(args.id, { layout, pipeline: gpu.createRenderPipeline(descriptor) });
        break;
      }
      case "compute": {
        const layout = objects.get(args.layout);
        objects.set(args.id, { layout, pipeline: gpu.createComputePipeline({
          label: args.label, layout: layout.layout, compute: objects.get(args.shader),
        }) });
        break;
      }
      case "writeBuffer": gpu.queue.writeBuffer(objects.get(args.id), args.offset, bytes); break;
      case "writeTexture": gpu.queue.writeTexture({ texture: objects.get(args.id), mipLevel: args.mipLevel, origin: args.origin },
        bytes, { bytesPerRow: args.bytesPerRow }, [...args.extent, 1]); break;
      case "submit": {
        const encoder = gpu.createCommandEncoder();
        const transient = [];
        let readback;
        try {
          const queries = args.measureGpu && gpu.features.has("timestamp-query")
            ? gpu.createQuerySet({ type: "timestamp", count: 2 }) : null;
          if (queries) {
            transient.push(queries);
            encoder.beginComputePass({
              timestampWrites: { querySet: queries, beginningOfPassWriteIndex: 0 },
            }).end();
          }
          for (const pass of args.passes) encodePass(encoder, objects, pass, transient);
          if (queries) {
            encoder.beginComputePass({
              timestampWrites: { querySet: queries, endOfPassWriteIndex: 1 },
            }).end();
            const resolved = gpu.createBuffer({
              size: 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
            });
            transient.push(resolved);
            readback = gpu.createBuffer({
              size: 16, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
            });
            transient.push(readback);
            encoder.resolveQuerySet(queries, 0, 2, resolved, 0);
            encoder.copyBufferToBuffer(resolved, 0, readback, 0, 16);
          }
          gpu.queue.submit([encoder.finish()]);
        } catch (error) {
          for (const buffer of transient) buffer.destroy();
          throw error;
        }
        complete(args.completion, transient, readback);
        break;
      }
      case "destroy": {
        const object = objects.get(args.id);
        if (args.kind === "surface") object?.unconfigure();
        else if (args.kind === "buffer" || args.kind === "texture") object?.destroy();
        objects.delete(args.id);
        break;
      }
      default: throw new Error(`Unknown WebGPU RHI operation ${op}`);
    }
  }

  return {
    get adapter() { return adapter; },
    get lost() { return lost; },
    async initialize() {
      adapter = await navigator.gpu?.requestAdapter();
      if (!adapter) throw new Error("No WebGPU adapter");
      const requiredFeatures = adapter.features.has("timestamp-query") ? ["timestamp-query"] : [];
      gpu = await adapter.requestDevice({ requiredFeatures });
      gpu.addEventListener("uncapturederror", event => { fault = event.error; console.error("WebGPU RHI:", fault.message); });
      gpu.lost.then(info => { lost = true; console.warn("WebGPU RHI:", info.message); });
      return navigator.gpu.getPreferredCanvasFormat();
    },
    imports: {
      request(pointer, size, data, dataSize) {
        const memory = wasm().memory.buffer;
        const { device, op, args } = JSON.parse(decoder.decode(new Uint8Array(memory, pointer >>> 0, size)));
        if (!gpu) return -1;
        if (op === "close") {
          devices.delete(device);
          return 0;
        }
        if (lost) return -2;
        if (fault && op !== "destroy") return -3;
        if (op === "open") {
          devices.set(device, new Map());
          return 0;
        }
        try {
          execute(devices.get(device), op, args, new Uint8Array(memory, data >>> 0, dataSize));
          return 0;
        } catch (error) {
          console.error(`WebGPU RHI ${op}:`, error);
          return -3;
        }
      },
    },
  };
}
