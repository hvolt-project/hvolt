import { first } from './a';

export function second(): number {
  return 2;
}

export function loop(): number {
  return first();
}
