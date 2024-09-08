# Simple QUIC client
## Usage


<details close>
<summary><strong>devnet</strong></summary>


> run 3 attempts in devnet
> ```sh
> cargo run -- --devnet --retry 3
> ```

</details>



<details open>
<summary><strong>mainnet</strong></summary>

> run 3 attempts in mainnet
> ```sh
> cargo run -- --mainnet --retry 3
> ```

</details>

<details close>
<summary><strong>helios-mainnet</strong></summary>

> run 3 attempts in helios mainnet
> ```sh
> cargo run -- --helios-mainnet --retry 3
> ```

</details>

---

<details close>
<summary>Using <code>make</code></summary>

<details close>
<summary><strong>devnet</strong></summary>


> run 3 attempts in devnet
> ```sh
> make devnet 3
> ```

</details>



<details close>
<summary><strong>mainnet</strong></summary>

> run 3 attempts in mainnet
> ```sh
> make mainnet 3
> ```

</details>

<details close>
<summary><strong>helios-mainnet</strong></summary>

> run 3 attempts in mainnet
> ```sh
> make helios_mainnet 3
> ```

</details>

</details>


## Problem

Code works perfectly on the _devnet_, but when switching to the _mainnet_, the transaction sending fails with a `ConnectionError(TimedOut)` error.

**The most interesting thing is that sometimes the transaction is successful.**

---

One guy said:
> _Unfortunately, this is a result of congestion on mainnet. Without stake, you're allocated very few connections to the validators in the network, which means that you'll often get timeouts when trying to send transactions._

> _There are improvements in flight on the server side to better handle connection spam, but it will take some time for it to be rolled out. And if Helius isn't working, you may want to ask their support team_
