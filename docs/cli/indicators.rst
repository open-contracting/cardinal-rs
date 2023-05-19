indicators
==========

.. code:: console

   $ ocdscardinal help indicators
   Calculate procurement indicators from OCDS compiled releases in a line-delimited JSON file

   The result is a JSON object, in which keys are OCIDs and values are results.

   Usage: ocdscardinal[EXE] indicators [OPTIONS] <FILE>

   Arguments:
     <FILE>
             The path to the file (or "-" for standard input), in which each line is a contracting
             process as JSON text

   Options:
     -c, --count
             Print the number of OCIDs with results

     -v, --verbose...
             Increase verbosity

     -s, --settings <SETTINGS>
             The path to the settings file

     -h, --help
             Print help (see a summary with '-h')

For a given indicator, a contracting process is excluded if:

-  The ``ocid`` isn’t a string.

-  The relevant organization references don’t set an ``id``.

-  The relevant fields aren’t of the correct type. `#10 <https://github.com/open-contracting/cardinal-rs/issues/10>`__ `#13 <https://github.com/open-contracting/cardinal-rs/issues/13>`__

-  Monetary values, where relevant, don’t use the main currency. `#11 <https://github.com/open-contracting/cardinal-rs/issues/11>`__

   To configure the main currency, add to the top of your settings file:

   .. code:: ini

      currency = USD

   Otherwise, the main currency is set to the first observed currency.

Terminology
-----------

A bid is “submitted” if its status is pending, valid (i.e. qualified), or disqualified. It is not submitted if its status is invited or withdrawn.

NF024 The percentage difference between the winning bid and the second-lowest valid bid is a low outlier
--------------------------------------------------------------------------------------------------------

For each contracting process, the difference is calculated as :math:`(secondLowestValidBidAmount - winningBidAmount) \over winningBidAmount`. A contracting process is flagged if the difference is less than the lower fence of :math:`Q_1 - 1.5(IQR)`, where :math:`Q_1` is the first quartile and :math:`IQR` is the interquartile range for the set of differences.

To configure the lower fence, add to your settings file:

.. code:: ini

   [NF024]
   threshold = 0.05

The indicator’s value is the percentage difference.

A contracting process is excluded if:

-  An award’s status is pending or invalid.
-  The winning bid is not the lowest bid. (This indicator requires the award criteria to be price-only.)
-  There are multiple active awards (a.k.a. winning bids). `#14 <https://github.com/open-contracting/cardinal-rs/issues/14>`__
-  A bid is submitted by multiple tenderers. `#17 <https://github.com/open-contracting/cardinal-rs/issues/17>`__
-  An award is made to multiple suppliers. `#17 <https://github.com/open-contracting/cardinal-rs/issues/17>`__

NF025 The ratio of winning bids to submitted bids for a top tenderer is a low outlier
-------------------------------------------------------------------------------------

For each tenderer, the ratio is calculated as :math:`numberOfWinningBids \over numberOfValidBids` across all contracting processes. A tenderer is flagged if its number of valid bids is greater than the upper fence of the third quartile of the set of numbers of valid bids, and if its ratio is less than the lower fence of :math:`Q_1 - 1.5(IQR)`, where :math:`Q_1` is the first quartile and :math:`IQR` is the interquartile range for the set of ratios.

To configure the upper fence, add to your settings file:

.. code:: ini

   [NF025]
   percentile = 75 # default

To configure the lower fence, add to your settings file:

.. code:: ini

   [NF025]
   threshold = 0.05

The indicator’s value is the ratio.

A contracting process is excluded if:

-  An award’s status is pending or invalid.
-  There are multiple active awards (a.k.a. winning bids). `#14 <https://github.com/open-contracting/cardinal-rs/issues/14>`__
-  A bid is submitted by multiple tenderers. `#17 <https://github.com/open-contracting/cardinal-rs/issues/17>`__
-  An award is made to multiple suppliers. `#17 <https://github.com/open-contracting/cardinal-rs/issues/17>`__

NF035 Bids are disqualified if not submitted by the single tenderer of the winning bid
--------------------------------------------------------------------------------------

A contracting process is flagged if:

-  Exactly one tenderer submitted one or more bids that are valid (i.e. qualified).

-  The tenderer of the valid bids and the suppliers of all active awards are the same.

-  At least 1 other tenderer submitted a bid that was disqualified.

   To configure this threshold, add to your settings file:

   .. code:: ini

      [NF035]
      threshold = 1 # default

The indicator’s value is the number of unique tenderers with disqualified bids.

A contracting process is excluded if:

-  An award’s status is pending or invalid.

NF036 The lowest submitted bid is disqualified, while the award criterion is price only
---------------------------------------------------------------------------------------

A contracting process is flagged if:

-  There are one or more active awards.
-  The lowest submitted bid is disqualified.

The indicator’s value is always 1.0.

NF038 The ratio of disqualified bids to submitted bids is a high outlier per buyer, procuring entity or tenderer
----------------------------------------------------------------------------------------------------------------

For each buyer, the ratio is calculated as :math:`numberOfBidsDisqualifiedByBuyer \over numberOfBidsSubmittedToBuyer` across all contracting processes. A buyer is flagged if its ratio is greater than the upper fence of :math:`Q_3 + 1.5(IQR)`, where :math:`Q_3` is the third quartile and :math:`IQR` is the interquartile range for the set of ratios. The same calculation is performed for procuring entities.

For each tenderer, the ratio is calculated as :math:`numberOfBidsDisqualifiedForTenderer \over numberOfBidsSubmittedByTenderer` across all contracting processes. A tenderer is flagged if its ratio is greater than the upper fence of :math:`Q_3 + 1.5(IQR)`, where :math:`Q_3` is the third quartile and :math:`IQR` is the interquartile range for the set of ratios.

To configure the upper fence, add to your settings file:

.. code:: ini

   [NF038]
   threshold = 0.5

The indicator’s value is the ratio.

This indicator assumes that ``buyer/id``, ``procuringEntity/id`` and ``bids/details/tenderers/id`` are stable across contracting processes. `#32 <https://github.com/open-contracting/cardinal-rs/issues/32>`__
