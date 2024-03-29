// Settings
cbuffer SETTINGS : register(b0) {
    float2 resolution;
    uint num_agents;
    uint steps_per_tick;

    float agent_speed;
    float agent_turn_rate_rad;
    float sensor_angle_rad;
    float sensor_offset;
    int sensor_size;
    float4 agent_color;
    float same_color_weight;
    float different_color_weight;

    float eat_weight;
    float trail_weight;
    float diffuse_rate;
    float exponential_decay_rate;
    float linear_decay_rate;

    // Time
    float time;
    float delta_time;
}

// Data
RWTexture2D<float4> trail: register(u0);
RWTexture2D<float4> diffused_trail: register(u1);

struct Agent
{
    float4 color;
    float2 position;
    float heading;
};
RWStructuredBuffer<Agent> agents: register(u2);

uint rand_uint(uint state)
{
    state ^= 2747636419u;
    state *= 2654435769u;
    state ^= state >> 16;
    state *= 2654435769u;
    state ^= state >> 16;
    state *= 2654435769u;
    return state;
}

float rand_float(uint state)
{
    return state / 4294967295.0;
}

float mod(const float x, const float y)
{
    return x - y * floor(x/y);
}

float2 mod2(const float2 x, const float2 y)
{
    return float2(mod(x.x, y.x), mod(x.y, y.y));
}

float sense(Agent agent, float angle_offset, float sensor_offset)
{
    float sensor_angle = agent.heading + angle_offset;
    float2 sensor_dir;
    sincos(sensor_angle, sensor_dir.y, sensor_dir.x);

    float sum = 0;
    
    for (int offset_x = -sensor_size; offset_x <= sensor_size; offset_x++)
    {
        for (int offset_y = -sensor_size; offset_y <= sensor_size; offset_y++)
        {
            float2 sensor_pos = mod2(agent.position + sensor_dir * sensor_offset + float2(offset_x, offset_y), resolution);
            sum += same_color_weight * dot(trail[sensor_pos], float4(agent.color.xyz, 0));
            float4 inv_color = 1 - agent.color;
            sum += different_color_weight * dot(trail[sensor_pos], inv_color);
        }
    }
    return sum;
}

[numthreads(32, 1, 1)]
void advance_agents (uint3 id : SV_DispatchThreadID)
{
    if (id.x >= num_agents)
        return;
    
    Agent agent = agents[id.x];
    
    // Adjust direction
    float weightF = sense(agent, 0, sensor_offset);
    float weightL = sense(agent, sensor_angle_rad, sensor_offset);
    float weightR = sense(agent, -sensor_angle_rad, sensor_offset);
    float turn_dir = 0;

    if (weightL < weightF && weightF < weightR)
    {
        turn_dir = -1;
    }
    else if (weightL > weightF && weightF > weightR)
    {
        turn_dir = 1;
    }
    else if (weightL < weightF && weightF > weightR)
    {
        turn_dir = 0;
    }
    else if (weightL > weightF && weightF < weightR)
    {
        turn_dir = sign(rand_float(time + id.x) - 0.5);
    }

    // float2 gradient = float2(0, 1);
    agent.heading += turn_dir * agent_turn_rate_rad; // * delta_time;
     
    // Eat
    trail[agent.position] = trail[agent.position] - agent.color * eat_weight * delta_time;

    // Move in direction
    float2 dir_vec;
    sincos(agent.heading, dir_vec.y, dir_vec.x);
    agent.position += agent_speed * dir_vec * delta_time;
    agent.position = mod2(agent.position, resolution);

    // trail[agent.position] += agent.color * trail_weight * delta_time;
    trail[agent.position] = trail[agent.position] + agent.color * trail_weight * delta_time;
    agents[id.x] = agent;
}

[numthreads(8, 8, 1)]
void decay_and_diffuse (uint3 id : SV_DispatchThreadID)
{
    if (id.x > (uint) resolution.x || id.y > (uint) resolution.y)
        return;
    
    const float diffuse_weight = saturate(diffuse_rate * delta_time);
    const float exp_decay_weight = saturate(exponential_decay_rate * delta_time);
    const float lin_decay_weight = max(0, linear_decay_rate * delta_time);

    float4 sum = 0;
    
    for (int offsetX = -1; offsetX <= 1; offsetX++)
    {
        for (int offsetY = -1; offsetY <= 1; offsetY++)
        {
            float2 sampleidx = mod2(id.xy + float2(offsetX, offsetY), resolution);
            sum += trail[sampleidx];
        }
    }

    float4 v = trail[id.xy] * (1 - diffuse_weight) + sum / 9 * diffuse_weight;
    diffused_trail[id.xy] = v*(1 - exp_decay_weight) - lin_decay_weight;
}