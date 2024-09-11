use alloy::primitives::U256;
use angstrom_types::{
    matching::Ray,
    orders::{OrderId, OrderPriorityData},
    sol_bindings::{
        grouped_orders::{GroupedVanillaOrder, OrderWithStorageData},
        rpc_orders::PartialFlashOrder,
        sol::FlashOrder
    }
};
use rand_distr::{Distribution, SkewNormal};

#[allow(clippy::too_many_arguments)]
pub fn order_distribution(
    is_bid: bool,
    number: usize,
    price_location: f64,
    price_scale: f64,
    price_shape: f64,
    quantity_location: f64,
    quantity_scale: f64,
    quantity_shape: f64
) -> Result<Vec<OrderWithStorageData<GroupedVanillaOrder>>, String> {
    let mut rng = rand::thread_rng();
    let mut rng2 = rand::thread_rng();
    let price_gen = SkewNormal::new(price_location, price_scale, price_shape)
        .map_err(|e| format!("Error creating price distribution: {}", e))?;
    let quantity_gen = SkewNormal::new(quantity_location, quantity_scale, quantity_shape)
        .map_err(|e| format!("Error creating price distribution: {}", e))?;
    Ok(price_gen
        .sample_iter(&mut rng)
        .zip(quantity_gen.sample_iter(&mut rng2))
        .map(|(p, q)| {
            let order = GroupedVanillaOrder::KillOrFill(
                angstrom_types::sol_bindings::grouped_orders::FlashVariants::Partial(
                    PartialFlashOrder {
                        maxAmountIn: q.floor() as u128,
                        minPrice: Ray::from(p).into(),
                        ..Default::default()
                    }
                )
            );
            OrderWithStorageData {
                invalidates: vec![],
                order,
                priority_data: OrderPriorityData {
                    price:  p as u128,
                    volume: q as u128,
                    gas:    0
                },
                is_bid,
                is_valid: true,
                is_currently_valid: true,
                order_id: OrderId::default(),
                pool_id: 0,
                valid_block: 0
            }
        })
        .take(number)
        .collect())
}
