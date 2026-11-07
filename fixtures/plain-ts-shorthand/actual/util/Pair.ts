export interface Bag {
	destination: string;
	gateway: string;
}
export function makeBag(): Bag {
  const destination = 'd';
  const gateway = 'g';
  return { destination: destination, gateway: gateway };
}
export function makeShorthandBag(): Bag {
  const destination = 'd2';
  const gateway = 'g2';
  return { destination, gateway };
}
